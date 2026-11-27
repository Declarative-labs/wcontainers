#![no_std]

use vstd::prelude::*;
#[cfg(verus_keep_ghost)]
use vstd::std_specs::iter::IteratorSpec;
use wcontainers_processor_architecture::{ExecutionState, ProcessorArchitecture};

verus! {

#[allow(reviewable::trait_not_verified, reason = "the contract is not written yet")]
pub trait DistinguishingStateGenerator<Architecture: ProcessorArchitecture> {
    /// The states generated for the instruction, in order.
    spec fn states(&self, instruction: Architecture::Instruction) -> Seq<ExecutionState<'static, Architecture>>;

    // VERUS: no `impl Trait` in a trait, so the returned iterator is a named type.
    type States<'generator>: Iterator<Item = ExecutionState<'static, Architecture>>
    where
        Self: 'generator;

    /// Returns a set of execution states that can be used to distinguish between all possible behaviors of the given instruction.
    fn generate_states(
        &self,
        instruction: Architecture::Instruction,
    ) -> (states: Self::States<'_>)
        ensures
            states.obeys_prophetic_iter_laws(),
            states.remaining() == self.states(instruction),
            states.decrease() is Some,
    ;
}

} // verus!
