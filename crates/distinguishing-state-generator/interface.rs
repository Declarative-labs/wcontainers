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

/// Every two different known behaviors of the instruction leave different states after some state the generator makes
/// for it. This is a definition, which no generator claims: whether it holds depends on the instructions' inputs.
pub open spec fn distinguishes_known_behaviors<Architecture, Generator>(
    generator: &Generator,
    instruction: Architecture::Instruction,
) -> bool
    where
        Architecture: ProcessorArchitecture,
        Architecture::PreciseInstruction: Simulator<Architecture, Error = Infallible>,
        Generator: DistinguishingStateGenerator<Architecture> + ?Sized,
{
    forall|first, second|
        #![trigger Architecture::known_behaviors(instruction).contains(first), Architecture::known_behaviors(instruction).contains(second)]
        Architecture::known_behaviors(instruction).contains(first) && Architecture::known_behaviors(instruction).contains(second)
            && first != second ==> exists|index: int| 0 <= index < generator.states(instruction).len()
                && #[trigger] simulated_after(first, generator.states(instruction)[index])
                    != simulated_after(second, generator.states(instruction)[index])
}

} // verus!
