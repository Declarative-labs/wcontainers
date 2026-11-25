//! A chip's precise instruction set as measurements give it, measured just now or read from a cache.

#![no_std]

extern crate alloc;

use alloc::vec::Vec;

#[cfg(verus_keep_ghost)]
use vstd::laws_eq::obeys_concrete_eq;
use vstd::prelude::*;
use wcontainers_precise_instruction_set::PreciseInstructionSet;
use wcontainers_processor_architecture::ProcessorArchitecture;

#[cfg(test)]
mod tests;

verus! {

/// Each measured instruction with the behavior measured for it.
pub struct MeasuredInstructionSet<Architecture: ProcessorArchitecture> {
    behaviors: Vec<(Architecture::Instruction, Architecture::PreciseInstruction)>,
}

/// The behavior of the first entry for the instruction.
pub open spec fn first_behavior<Instruction, Precise>(behaviors: Seq<(Instruction, Precise)>, instruction: Instruction) -> Precise
    decreases behaviors.len(),
{
    if behaviors.len() == 0 {
        arbitrary()
    } else if behaviors[0].0 == instruction {
        behaviors[0].1
    } else {
        first_behavior(behaviors.drop_first(), instruction)
    }
}

impl<Architecture: ProcessorArchitecture> PreciseInstructionSet<Architecture>
    for MeasuredInstructionSet<Architecture>
{
    closed spec fn behavior(&self, instruction: Architecture::Instruction) -> Architecture::PreciseInstruction {
        first_behavior(self.behaviors@, instruction)
    }

    fn from_measurements(
        measurements: &[(Architecture::Instruction, Architecture::PreciseInstruction)],
    ) -> Self {
        // VERUS: no `to_vec` of a slice, so the measurements are copied by index into a list made to hold them.
        let mut behaviors = Vec::with_capacity(measurements.len());
        let mut index = 0;
        while index < measurements.len()
            invariant
                index <= measurements@.len(),
                behaviors@ == measurements@.take(index as int),
            decreases measurements@.len() - index,
        {
            behaviors.push(measurements[index]);
            proof {
                assert(behaviors@ =~= measurements@.take(index + 1));
            }
            index += 1;
        }
        proof {
            lemma_first_pairs(measurements@, behaviors@);
        }
        Self { behaviors }
    }

    /// Panics for an instruction the measurements do not hold.
    fn to_precise(
        &self,
        instruction: Architecture::Instruction,
    ) -> Architecture::PreciseInstruction {
        proof {
            Architecture::equality_is_identity();
            assert(self.behaviors@.skip(0) =~= self.behaviors@);
        }
        // VERUS: no `find` with a closure, so the measurements are searched by index.
        let mut index = 0;
        while index < self.behaviors.len()
            invariant
                index <= self.behaviors@.len(),
                obeys_concrete_eq::<Architecture::Instruction>(),
                first_behavior(self.behaviors@.skip(index as int), instruction) == first_behavior(self.behaviors@, instruction),
            decreases self.behaviors@.len() - index,
        {
            let (measured, precise) = self.behaviors[index];
            proof {
                reveal(obeys_concrete_eq);
                assert(self.behaviors@.skip(index as int)[0] == (measured, precise));
                assert(self.behaviors@.skip(index as int).drop_first() =~= self.behaviors@.skip(index + 1));
            }
            if measured == instruction {
                return precise;
            }
            index += 1;
        }
        // VERUS: no panic it can rule out without a precondition the trait does not have, so the panic is a trusted
        // function that never returns.
        unmeasured()
    }
}

/// Panics: the measurements do not hold the instruction.
#[cfg_attr(verus_keep_ghost, verus::trusted)]
#[verifier::external_body]
fn unmeasured<Precise>() -> (never: Precise)
    ensures
        false,
{
    panic!("only measured instructions have a behavior")
}

/// The first entry for an instruction is the entry no earlier one names.
proof fn lemma_first_pair<Instruction, Precise>(behaviors: Seq<(Instruction, Precise)>, at: int)
    requires
        0 <= at < behaviors.len(),
        forall|earlier: int| 0 <= earlier < at ==> (#[trigger] behaviors[earlier]).0 != behaviors[at].0,
    ensures
        first_behavior(behaviors, behaviors[at].0) == behaviors[at].1,
    decreases at,
{
    if at > 0 {
        let rest = behaviors.drop_first();
        assert(behaviors[0].0 != behaviors[at].0);
        assert(rest[at - 1] == behaviors[at]);
        assert forall|earlier: int| 0 <= earlier < at - 1 implies (#[trigger] rest[earlier]).0 != rest[at - 1].0 by {
            assert(rest[earlier] == behaviors[earlier + 1]);
        }
        lemma_first_pair(rest, at - 1);
    }
}

/// A copy of the measurements gives each instruction the behavior of the first pair that names it.
proof fn lemma_first_pairs<Instruction, Precise>(measurements: Seq<(Instruction, Precise)>, behaviors: Seq<(Instruction, Precise)>)
    requires
        behaviors == measurements.take(measurements.len() as int),
    ensures
        forall|at: int| #![trigger measurements[at]] 0 <= at < measurements.len()
            && (forall|earlier: int| 0 <= earlier < at ==> (#[trigger] measurements[earlier]).0 != measurements[at].0)
            ==> first_behavior(behaviors, measurements[at].0) == measurements[at].1,
{
    assert(behaviors =~= measurements);
    assert forall|at: int| #![trigger measurements[at]] 0 <= at < measurements.len()
        && (forall|earlier: int| 0 <= earlier < at ==> (#[trigger] measurements[earlier]).0 != measurements[at].0)
        implies first_behavior(behaviors, measurements[at].0) == measurements[at].1 by {
        lemma_first_pair(measurements, at);
    }
}

} // verus!
