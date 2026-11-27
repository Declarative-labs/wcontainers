#![no_std]

use wcontainers_processor_architecture::ProcessorArchitecture;

/// One choice of behavior for every instruction of an architecture.
#[allow(reviewable::trait_not_verified, reason = "the contract is not written yet")]
pub trait PreciseInstructionSet<Architecture: ProcessorArchitecture>: Sized {
    fn from_measurements(
        measurements: &[(
            Architecture::Instruction,
            Architecture::PreciseInstruction,
        )],
    ) -> Self;

    /// Returns the instruction with the behavior this set gives it.
    fn to_precise(
        &self,
        instruction: Architecture::Instruction,
    ) -> Architecture::PreciseInstruction;
}
