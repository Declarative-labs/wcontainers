#![no_std]

use core::convert::Infallible;

use vstd::prelude::*;
use wcontainers_behavior_source::BehaviorSource;
#[cfg(verus_keep_ghost)]
use wcontainers_behavior_source::shows_alone;
use wcontainers_processor_architecture::{ProcessorArchitecture, Simulator};

verus! {

/// One choice of behavior for every instruction of an architecture.
pub trait PreciseInstructionSet<Architecture: ProcessorArchitecture>: Sized {
    /// What the behaviors are measured from, such as a live chip or a recording of one.
    type Source: BehaviorSource<Architecture = Architecture>;
    type MeasureError;

    /// The behavior this set gives the instruction.
    spec fn behavior(&self, instruction: Architecture::Instruction) -> Architecture::PreciseInstruction;

    /// Measures which behavior each instruction shows.
    // VERUS: no bound on the trait says known behaviors simulate, so measuring has that bound of its own.
    fn measure(source: Self::Source) -> (set: Result<Self, Self::MeasureError>)
    where
        Architecture::PreciseInstruction: Simulator<Architecture, Error = Infallible>,
        ensures
            set matches Ok(set) ==> forall|instruction| #[trigger] Architecture::instructions().contains(instruction)
                ==> shows_alone(&source, instruction, set.behavior(instruction)),
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
