#![no_std]

use wcontainers_processor_architecture::ProcessorArchitecture;

/// One choice of behaviour for every instruction of an architecture.
pub trait PreciseInstructionSet<Architecture: ProcessorArchitecture> {
    /// Returns the instruction with the behaviour this set gives it.
    fn to_precise(
        &self,
        instruction: Architecture::Instruction,
    ) -> Architecture::PreciseInstruction;
}
