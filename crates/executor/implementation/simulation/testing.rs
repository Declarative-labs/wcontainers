//! What the tests give the simulation: one known behavior of each instruction, as one chip shows them, and random bytes.

use std::cmp::Reverse;
use std::convert::Infallible;

use wcontainers_distinguishing_state_generator::DistinguishingStateGenerator;
use wcontainers_distinguishing_state_generator_arm64::CriticalValueCut;
use wcontainers_precise_instruction_set::PreciseInstructionSet;
use wcontainers_precise_instruction_set_measured::MeasuredInstructionSet;
use wcontainers_processor_architecture::{ExecutionState, ProcessorArchitecture, Simulator};
use wcontainers_processor_architecture_arm64::memory::read_bytes;
use wcontainers_processor_architecture_arm64::{Arm64, Instruction, User};
use wcontainers_random_source::StandaloneRandomSource;
use wcontainers_random_source_seeded::{SeededRandomSource, SeededRandomSourceConfig};

/// The seed of the values the simulation chooses where a behavior leaves them undecided.
pub const SEED: u64 = 0;

/// The behaviors the tests' simulation runs, one a chip shows for each instruction.
///
/// For each instruction it is the first the architecture lists that one chip shows together with the first behavior of
/// every more specific encoding.
pub type FirstBehaviors = MeasuredInstructionSet<Arm64<User>>;

/// Returns for every instruction with a known behavior the first that agrees with the more specific encodings.
///
/// It agrees where it gives what the first behavior of the most specific listed encoding gives on each generated state
/// whose word that encoding takes.
///
/// The simulation runs a word as the listed encoding that fixes most bits, as a chip does: so a HINT whose first
/// behavior is a chip's without FEAT_PAuth would disagree on hint 7, which XPACLRI's behavior strips. An instruction no
/// behavior of which agrees keeps its first.
pub fn list_first_behaviors() -> FirstBehaviors {
    let generator = CriticalValueCut::default();
    let chosen: Vec<_> = Arm64::<User>::list_instructions()
        .filter_map(|instruction| {
            let first = Arm64::<User>::list_known_behaviors(instruction).next()?;
            let taken = list_taken_states(instruction, &generator);
            let agreeing = Arm64::<User>::list_known_behaviors(instruction).find(|precise| {
                taken.iter().all(|(state, other)| {
                    precise.simulate_state(state.clone()) == other.simulate_state(state.clone())
                })
            });
            Some((instruction, agreeing.unwrap_or(first)))
        })
        .collect();
    FirstBehaviors::from_measurements(&chosen)
}

/// Lists the generated states of the instruction that a more specific listed encoding takes, with its first behavior.
///
/// An encoding takes a state's word where it has a known behavior and fixes more bits than the instruction does.
fn list_taken_states(
    instruction: Instruction<Infallible>,
    generator: &CriticalValueCut,
) -> Vec<(
    ExecutionState<'static, Arm64<User>>,
    <Arm64<User> as ProcessorArchitecture>::PreciseInstruction,
)> {
    let own = instruction.to_fixed_bits();
    let more_specific: Vec<_> = Arm64::<User>::list_instructions()
        .filter(|other| {
            let fixed = other.to_fixed_bits();
            *other != instruction
                && fixed.mask.count_ones() > own.mask.count_ones()
                && fixed.mask & own.mask == own.mask
                && fixed.value & own.mask == own.value
        })
        .collect();
    if more_specific.is_empty() {
        return Vec::new();
    }
    generator
        .generate_states(instruction)
        .filter_map(|state| {
            let word = u32::from_le_bytes(read_bytes(
                &state.memory_mutations,
                state.processor.program_counter,
            ));
            let taker = more_specific
                .iter()
                .filter(|other| {
                    let fixed = other.to_fixed_bits();
                    word & fixed.mask == fixed.value
                })
                .min_by_key(|other| Reverse(other.to_fixed_bits().mask.count_ones()))?;
            let behavior = Arm64::<User>::list_known_behaviors(*taker).next()?;
            Some((state, behavior))
        })
        .collect()
}

/// Returns the tests' random source, which gives the same words in the same order for the same seed.
pub fn seed_random(seed: u64) -> SeededRandomSource {
    SeededRandomSource::new(SeededRandomSourceConfig { seed })
}
