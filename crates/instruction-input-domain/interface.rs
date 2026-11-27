#![no_std]

extern crate alloc;

use alloc::vec::{self, Vec};

use itertools::{Itertools, MultiProduct};
use vstd::prelude::*;
use wcontainers_processor_architecture::{ExecutionState, ProcessorArchitecture};

verus! {

pub trait InstructionInputDomain<Architecture: ProcessorArchitecture>: Sized {
    /// The type of one input of an instruction, which does not depend on the types of its other inputs.
    type OperandType: Copy + Eq;
    type OperandValue: Copy;
    // VERUS: no `impl Trait` in a trait, so each returned iterator is a named type.
    type OperandTypes<'domain>: Iterator<Item = Self::OperandType>
    where
        Self: 'domain;
    type CriticalValues<'domain>: Iterator<Item = Self::OperandValue>
    where
        Self: 'domain;

    /// Returns the type of each of the instruction's operands, in the order of its signature.
    fn list_operand_types(&self, instruction: Architecture::Instruction) -> Self::OperandTypes<'_>
        // TODO: replace this clause with what the method promises.
        ensures
            true,
    ;

    /// Returns the values of the type at which the behavior of an instruction can change.
    fn generate_critical_values(&self, operand_type: Self::OperandType)
    -> Self::CriticalValues<'_>
        // TODO: replace this clause with what the method promises.
        ensures
            true,
    ;

    /// Returns an execution state in which the next instruction executed is the given instruction with the given operands.
    ///
    /// The operands are in the order of the instruction's signature.
    fn build_state(
        &self,
        instruction: Architecture::Instruction,
        operands: &[Self::OperandValue],
    ) -> ExecutionState<'static, Architecture>
        // TODO: replace this clause with what the method promises.
        ensures
            true,
    ;

    /// Returns the state of every combination of the critical values of the instruction's operands.
    fn generate_critical_states(
        &self,
        instruction: Architecture::Instruction,
    ) -> CriticalStates<'_, Self, Architecture>
        // TODO: replace this clause with what the method promises.
        ensures
            true,
    {
        CriticalStates {
            domain: self,
            instruction,
            combinations: self
                .list_operand_types(instruction)
                .map(|operand_type| {
                    self.generate_critical_values(operand_type)
                        .collect::<Vec<_>>()
                })
                .multi_cartesian_product(),
        }
    }
}

/// The state of every combination of the critical values of an instruction's operands.
// VERUS: no name for an iterator built from closures, so the states are built by a type of the interface.
pub struct CriticalStates<'domain, Domain, Architecture>
where
    Domain: InstructionInputDomain<Architecture>,
    Architecture: ProcessorArchitecture,
{
    pub domain: &'domain Domain,
    pub instruction: Architecture::Instruction,
    pub combinations: MultiProduct<vec::IntoIter<Domain::OperandValue>>,
}

} // verus!

impl<Domain, Architecture> Iterator for CriticalStates<'_, Domain, Architecture>
where
    Domain: InstructionInputDomain<Architecture>,
    Architecture: ProcessorArchitecture,
{
    type Item = ExecutionState<'static, Architecture>;

    fn next(&mut self) -> Option<Self::Item> {
        let operands = self.combinations.next()?;
        Some(self.domain.build_state(self.instruction, &operands))
    }
}
