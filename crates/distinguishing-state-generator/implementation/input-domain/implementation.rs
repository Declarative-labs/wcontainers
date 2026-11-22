//! A generator that forms every combination of the critical values of an instruction's operands.

#![no_std]

use wcontainers_distinguishing_state_generator::DistinguishingStateGenerator;
use wcontainers_instruction_input_domain::InstructionInputDomain;
use wcontainers_processor_architecture::{ExecutionState, ProcessorArchitecture};

#[derive(Clone, Copy, Debug)]
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
    Architecture: ProcessorArchitecture,
    Domain: InstructionInputDomain<Architecture>,
{
    fn generate_states(
        &self,
        instruction: Architecture::Instruction,
    ) -> impl Iterator<Item = ExecutionState<'static, Architecture>> {
        self.domain.generate_critical_states(instruction)
    }
}
