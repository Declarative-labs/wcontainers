#![no_std]

use vstd::prelude::*;
use wcontainers_behavior_source::BehaviorSource;
use wcontainers_processor_architecture::ProcessorArchitecture;

verus! {

/// One choice of behavior for every instruction of an architecture.
pub trait PreciseInstructionSet<Architecture: ProcessorArchitecture>: Sized {
    /// What the behaviors are measured from, such as a live chip or a recording of one.
    type Source: BehaviorSource<Architecture = Architecture>;
    type MeasureError;

    /// The behavior this set gives the instruction.
    spec fn behavior(&self, instruction: Architecture::Instruction) -> Architecture::PreciseInstruction;

    /// Measures which behavior each instruction shows.
    fn measure(source: Self::Source) -> (set: Result<Self, Self::MeasureError>)
        ensures
            set matches Ok(set) ==> forall|instruction| #[trigger] Architecture::instructions().contains(instruction)
                ==> Architecture::known_behaviors(instruction).contains(set.behavior(instruction)),
    ;

    /// Returns the instruction with the behavior this set gives it.
    fn to_precise(&self, instruction: Architecture::Instruction) -> (precise: Architecture::PreciseInstruction)
        ensures
            precise == self.behavior(instruction),
    ;
}

} // verus!
