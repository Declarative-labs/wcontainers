#![no_std]

use wcontainers_behavior_source::BehaviorSource;
use wcontainers_processor_architecture::ProcessorArchitecture;

/// One choice of behavior for every instruction of an architecture.
pub trait PreciseInstructionSet<Architecture: ProcessorArchitecture>: Sized {
    /// What the behaviors are measured from, such as a live chip or a recording of one.
    type Source: BehaviorSource<Architecture = Architecture>;
    type MeasureError;

    /// Measures which behavior each instruction shows.
    fn measure(source: Self::Source) -> Result<Self, Self::MeasureError>;

    /// Returns the instruction with the behavior this set gives it.
    fn to_precise(
        &self,
        instruction: Architecture::Instruction,
    ) -> Architecture::PreciseInstruction;
}
