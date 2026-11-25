//! The precise instruction set of a chip, found by running every generated state on a source of its behavior and
//! keeping the one known behavior that agrees with it.

#![no_std]

extern crate alloc;

use alloc::vec::Vec;
use core::convert::Infallible;
use core::marker::PhantomData;

use wcontainers_behavior_source::{BehaviorSource, MeasureInstructionError};
use wcontainers_distinguishing_state_generator::DistinguishingStateGenerator;
use wcontainers_precise_instruction_set::PreciseInstructionSet;
use wcontainers_processor_architecture::{ProcessorArchitecture, Simulator};

#[cfg(test)]
mod tests;

type InstructionOf<Source> =
    <<Source as BehaviorSource>::Architecture as ProcessorArchitecture>::Instruction;
type PreciseInstructionOf<Source> =
    <<Source as BehaviorSource>::Architecture as ProcessorArchitecture>::PreciseInstruction;

/// Each listed instruction with the behavior a source showed on every state the generator it was measured with made.
pub struct MeasuredInstructionSet<Source: BehaviorSource> {
    behaviors: Vec<(InstructionOf<Source>, PreciseInstructionOf<Source>)>,
    measured_from: PhantomData<Source>,
}

impl<Source> PreciseInstructionSet<Source::Architecture> for MeasuredInstructionSet<Source>
where
    Source: BehaviorSource,
    Source::Architecture: Clone,
    PreciseInstructionOf<Source>: Simulator<Source::Architecture, Error = Infallible>,
{
    type Source = Source;
    type MeasureError = MeasureInstructionError<Source::Architecture, Source::Error>;

    fn measure(
        mut source: Source,
        generator: &impl DistinguishingStateGenerator<Source::Architecture>,
    ) -> Result<Self, Self::MeasureError> {
        let behaviors = Source::Architecture::list_instructions()
            .map(|instruction| {
                let precise = source.measure_instruction(instruction, generator)?;
                Ok((instruction, precise))
            })
            .collect::<Result<_, _>>()?;
        Ok(Self {
            behaviors,
            measured_from: PhantomData,
        })
    }

    /// Panics for an instruction that `list_instructions` does not list.
    fn to_precise(&self, instruction: InstructionOf<Source>) -> PreciseInstructionOf<Source> {
        let measured = self
            .behaviors
            .iter()
            .find(|(measured, _)| *measured == instruction);
        match measured {
            Some((_, precise)) => *precise,
            None => panic!("only listed instructions are measured"),
        }
    }
}
