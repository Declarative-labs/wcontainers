#![no_std]

use vstd::prelude::*;
use wcontainers_processor_architecture::ProcessorArchitecture;

verus! {

/// One choice of behavior for every instruction of an architecture.
pub trait PreciseInstructionSet<Architecture: ProcessorArchitecture>: Sized {
    fn from_measurements(
        measurements: &[(
            Architecture::Instruction,
            Architecture::PreciseInstruction,
        )],
    ) -> Self
        // TODO: replace this clause with what the method promises.
        ensures
            true,
    ;

    /// Returns the instruction with the behavior this set gives it.
    fn to_precise(
        &self,
        instruction: Architecture::Instruction,
    ) -> Architecture::PreciseInstruction
        // TODO: replace this clause with what the method promises.
        ensures
            true,
    ;
}

} // verus!
