//! The precise instruction sets of the kinds of cores whose samples are baked into this crate.
//!
//! Each folder of `samples/` holds one architecture's baked files, one for each kind of core; `build.rs` lists them,
//! and the module of the folder names the architecture and core identity of its samples.

#![no_std]

extern crate alloc;

use alloc::vec::Vec;

use wcontainers_precise_instruction_set::PreciseInstructionSet;
use wcontainers_processor_architecture::ProcessorArchitecture;

mod baked;

pub use baked::BakedSample;
#[cfg(feature = "bake")]
pub use baked::{BakeSampleError, bake_sample};

/// The samples of the accumulator machine, the architecture that tests what a baked file holds.
#[cfg(test)]
mod accumulator_machine {
    use wcontainers_processor_architecture_accumulator_machine::{
        AccumulatorMachineProcessorArchitecture, CoreIdentity,
    };

    type Sample = crate::BakedSample<AccumulatorMachineProcessorArchitecture, CoreIdentity>;

    include!(concat!(env!("OUT_DIR"), "/accumulator-machine.rs"));
}

/// Each instruction of a sample with the behavior its kind of core shows.
pub struct CollectedPreciseInstructionSet<Architecture: ProcessorArchitecture> {
    measurements: Vec<(Architecture::Instruction, Architecture::PreciseInstruction)>,
}

impl<Architecture: ProcessorArchitecture> PreciseInstructionSet<Architecture>
    for CollectedPreciseInstructionSet<Architecture>
{
    fn from_measurements(
        measurements: &[(Architecture::Instruction, Architecture::PreciseInstruction)],
    ) -> Self {
        Self {
            measurements: Vec::from(measurements),
        }
    }

    /// Panics for an instruction the measurements do not hold.
    fn to_precise(
        &self,
        instruction: Architecture::Instruction,
    ) -> Architecture::PreciseInstruction {
        let measured = self
            .measurements
            .iter()
            .find(|(measured, _)| *measured == instruction);
        match measured {
            Some((_, precise)) => *precise,
            None => panic!("only sampled instructions have a behavior"),
        }
    }
}

#[cfg(test)]
mod tests;
