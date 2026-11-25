//! A generator that forms every combination of the critical values of an instruction's operands.

#![no_std]

extern crate alloc;

use alloc::boxed::Box;

use wcontainers_distinguishing_state_generator::DistinguishingStateGenerator;
use wcontainers_instruction_input_domain::InstructionInputDomain;
use wcontainers_processor_architecture::{ExecutionState, ProcessorArchitecture};

#[derive(Clone, Copy, Default, Debug)]
pub struct CriticalValueProduct<Domain> {
    domain: Domain,
}

impl<Domain> CriticalValueProduct<Domain> {
    pub const fn new(domain: Domain) -> Self {
        Self { domain }
    }
}

impl<Architecture, Domain> DistinguishingStateGenerator<Architecture>
    for CriticalValueProduct<Domain>
where
    Architecture: ProcessorArchitecture + 'static,
    Domain: InstructionInputDomain<Architecture>,
{
    // The domain's states come from a method that returns `impl Iterator`, a type no name reaches, so they are boxed.
    type States<'generator>
        = Box<dyn Iterator<Item = ExecutionState<'static, Architecture>> + 'generator>
    where
        Self: 'generator;

    fn generate_states(&self, instruction: Architecture::Instruction) -> Self::States<'_> {
        Box::new(self.domain.generate_critical_states(instruction))
    }
}
