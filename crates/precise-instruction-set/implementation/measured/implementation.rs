//! The precise instruction set of a chip as measurements give it: the behavior measured for each instruction, as a
//! source measured it or as a cache of such measurements holds it.

#![no_std]

extern crate alloc;

use alloc::vec::Vec;

use wcontainers_precise_instruction_set::PreciseInstructionSet;
use wcontainers_processor_architecture::ProcessorArchitecture;

#[cfg(test)]
mod tests;

/// Each measured instruction with the behavior measured for it.
pub struct MeasuredInstructionSet<Architecture: ProcessorArchitecture> {
    behaviors: Vec<(Architecture::Instruction, Architecture::PreciseInstruction)>,
}

impl<Architecture: ProcessorArchitecture> PreciseInstructionSet<Architecture>
    for MeasuredInstructionSet<Architecture>
{
    fn from_measurements(
        measurements: &[(Architecture::Instruction, Architecture::PreciseInstruction)],
    ) -> Self {
        Self {
            behaviors: measurements.to_vec(),
        }
    }

    /// Panics for an instruction the measurements do not hold.
    fn to_precise(&self, instruction: Architecture::Instruction) -> Architecture::PreciseInstruction {
        let measured = self
            .behaviors
            .iter()
            .find(|(measured, _)| *measured == instruction);
        match measured {
            Some((_, precise)) => *precise,
            None => panic!("only measured instructions have a behavior"),
        }
    }
}
