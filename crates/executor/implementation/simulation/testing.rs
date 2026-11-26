//! What the tests give the simulation: the first known behavior of each instruction, and seeded random bytes.

use std::sync::atomic::{AtomicU64, Ordering};

use wcontainers_abi::HostError;
use wcontainers_precise_instruction_set::PreciseInstructionSet;
use wcontainers_precise_instruction_set_measured::MeasuredInstructionSet;
use wcontainers_processor_architecture::ProcessorArchitecture;
use wcontainers_processor_architecture_arm64::{Arm64, User};
use wcontainers_random_source::RandomSource;
use wcontainers_volatile_bytes::VolatileBytes;

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

/// The step of [SplitMix64](https://prng.di.unimi.it/splitmix64.c), the odd part of the golden ratio in 64 bits.
const STEP: u64 = 0x9e37_79b9_7f4a_7c15;
/// SplitMix64's first multiplier.
const FIRST_MULTIPLIER: u64 = 0xbf58_476d_1ce4_e5b9;
/// SplitMix64's second multiplier.
const SECOND_MULTIPLIER: u64 = 0x94d0_49bb_1331_11eb;
/// SplitMix64's three shifts, in the order it applies them.
const SHIFTS: [u32; 3] = [30, 27, 31];

/// A random source for tests: SplitMix64 from a seed, the same words in the same order for the same seed.
pub struct SeededRandom {
    state: AtomicU64,
}

impl SeededRandom {
    pub const fn new(seed: u64) -> Self {
        Self {
            state: AtomicU64::new(seed),
        }
    }

    /// Returns the next word of the sequence.
    fn next_word(&self) -> u64 {
        let [first, second, third] = SHIFTS;
        let mut word = self
            .state
            .fetch_add(STEP, Ordering::Relaxed)
            .wrapping_add(STEP);
        word = (word ^ (word >> first)).wrapping_mul(FIRST_MULTIPLIER);
        word = (word ^ (word >> second)).wrapping_mul(SECOND_MULTIPLIER);
        word ^ (word >> third)
    }
}

impl RandomSource for SeededRandom {
    fn fill_random_bytes(&self, into: &impl VolatileBytes) -> Result<(), HostError> {
        let length = usize::try_from(into.len()).map_err(|_| HostError::NotSupported)?;
        let mut bytes = Vec::with_capacity(length);
        while bytes.len() < length {
            bytes.extend_from_slice(&self.next_word().to_le_bytes());
        }
        into.write(&bytes);
        Ok(())
    }
}
