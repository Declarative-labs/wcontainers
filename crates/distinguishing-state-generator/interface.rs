#![no_std]

#[cfg(verus_keep_ghost)]
use core::convert::Infallible;

use vstd::prelude::*;
#[cfg(verus_keep_ghost)]
use vstd::std_specs::iter::IteratorSpec;
use wcontainers_processor_architecture::{ExecutionState, ProcessorArchitecture};
#[cfg(verus_keep_ghost)]
use wcontainers_processor_architecture::{Simulator, simulated_after};

verus! {

pub trait DistinguishingStateGenerator<Architecture: ProcessorArchitecture> {
    /// The states generated for the instruction, in order.
    spec fn states(&self, instruction: Architecture::Instruction) -> Seq<ExecutionState<'static, Architecture>>;

    // VERUS: no `impl Trait` in a trait, so the returned iterator is a named type.
    type States<'generator>: Iterator<Item = ExecutionState<'static, Architecture>>
    where
        Self: 'generator;

    /// Returns a set of execution states that can be used to distinguish between all possible behaviors of the given instruction.
    fn generate_states(&self, instruction: Architecture::Instruction) -> (states: Self::States<'_>)
        ensures
            states.obeys_prophetic_iter_laws(),
            states.remaining() == self.states(instruction),
            states.decrease() is Some,
    ;

    /// Every two different known behaviors of the instruction leave different states after some generated state.
    // VERUS: no bound on the trait says known behaviors simulate, so the property is a proof with that bound of its own.
    proof fn distinguishes_known_behaviors(&self, instruction: Architecture::Instruction)
        where
            Architecture::PreciseInstruction: Simulator<Architecture, Error = Infallible>,
        ensures
            forall|first, second|
                #![trigger Architecture::known_behaviors(instruction).contains(first), Architecture::known_behaviors(instruction).contains(second)]
                Architecture::known_behaviors(instruction).contains(first) && Architecture::known_behaviors(instruction).contains(second)
                    && first != second ==> exists|index: int| 0 <= index < self.states(instruction).len()
                        && #[trigger] simulated_after(first, self.states(instruction)[index])
                            != simulated_after(second, self.states(instruction)[index]),
    ;
}

} // verus!
