//! A generator that forms every combination of the critical values of an instruction's operands.

#![no_std]

use wcontainers_distinguishing_state_generator::DistinguishingStateGenerator;
use wcontainers_instruction_input_domain::{CriticalStates, InstructionInputDomain};
use wcontainers_processor_architecture::ProcessorArchitecture;

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
    type States<'generator>
        = CriticalStates<'generator, Domain, Architecture>
    where
        Self: 'generator;

    fn generate_states(&self, instruction: Architecture::Instruction) -> Self::States<'_> {
        self.domain.generate_critical_states(instruction)
    }
}
