#![no_std]

use vstd::prelude::*;
use wcontainers_processor_architecture::{ExecutionState, ProcessorArchitecture};


verus! {

pub trait DistinguishingStateGenerator<Architecture: ProcessorArchitecture> {
    // VERUS: no `impl Trait` in a trait, so the returned iterator is a named type.
    type States<'generator>: Iterator<Item = ExecutionState<'static, Architecture>>
    where
        Self: 'generator;

    /// Returns a set of execution states that can be used to distinguish between all possible behaviors of the given instruction.
    fn generate_states(
        &self,
        instruction: Architecture::Instruction,
    ) -> Self::States<'_>
        // TODO: replace this clause with what the method promises.
        ensures
            true,
    ;
}

} // verus!
