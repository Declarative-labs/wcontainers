#![no_std]

use wcontainers_processor_architecture::{ExecutionState, ProcessorArchitecture};


pub trait DistinguishingStateGenerator<Architecture: ProcessorArchitecture> {
    /// Returns a set of execution states that can be used to distinguish between all possible behaviors of the given instruction.
    fn generate_states(
        &self,
        instruction: Architecture::Instruction,
    ) -> impl Iterator<Item = ExecutionState<'static, Architecture>>;
}
