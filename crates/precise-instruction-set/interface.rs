#![no_std]

use vstd::prelude::*;
use wcontainers_processor_architecture::ProcessorArchitecture;

verus! {

/// One choice of behavior for every instruction of an architecture.
#[allow(reviewable::trait_not_verified, reason = "the contract is not written yet")]
pub trait PreciseInstructionSet<Architecture: ProcessorArchitecture>: Sized {
    /// The behavior this set gives the instruction.
    spec fn behavior(&self, instruction: Architecture::Instruction) -> Architecture::PreciseInstruction;

    fn from_measurements(
        measurements: &[(
            Architecture::Instruction,
            Architecture::PreciseInstruction,
        )],
    ) -> (set: Self)
        ensures
            // Each instruction has the behavior of the first pair that names it.
            forall|at: int| #![trigger measurements@[at]] 0 <= at < measurements@.len()
                && (forall|earlier: int| 0 <= earlier < at ==> (#[trigger] measurements@[earlier]).0 != measurements@[at].0)
                ==> set.behavior(measurements@[at].0) == measurements@[at].1,
    ;

    /// Returns the instruction with the behavior this set gives it.
    fn to_precise(
        &self,
        instruction: Architecture::Instruction,
    ) -> (precise: Architecture::PreciseInstruction)
        ensures
            precise == self.behavior(instruction),
    ;
}

} // verus!
