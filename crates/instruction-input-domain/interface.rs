#![no_std]

extern crate alloc;

use alloc::vec::{self, Vec};

// VERUS: `multi_cartesian_product` runs in a trusted function, so `Itertools` is imported there.
use itertools::MultiProduct;
use vstd::prelude::*;
#[cfg(verus_keep_ghost)]
use vstd::std_specs::iter::IteratorSpec;
use wcontainers_processor_architecture::{ExecutionState, ProcessorArchitecture};

mod trusted;
#[cfg(test)]
mod trusted_tests;

use trusted::map_multi_cartesian_product;
#[cfg(verus_keep_ghost)]
use trusted::product;

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

    /// The type of each of the instruction's operands, in the order of its signature.
    spec fn operand_types(&self, instruction: Architecture::Instruction) -> Seq<Self::OperandType>;

    /// The values of the type at which the behavior of an instruction can change.
    spec fn critical_values(&self, operand_type: Self::OperandType) -> Seq<Self::OperandValue>;

    /// The execution state built for the instruction with the operands.
    spec fn state(
        &self,
        instruction: Architecture::Instruction,
        operands: Seq<Self::OperandValue>,
    ) -> ExecutionState<'static, Architecture>;

    /// Returns the type of each of the instruction's operands, in the order of its signature.
    fn list_operand_types(&self, instruction: Architecture::Instruction) -> (operand_types: Self::OperandTypes<'_>)
        ensures
            operand_types.obeys_prophetic_iter_laws(),
            operand_types.remaining() == self.operand_types(instruction),
            operand_types.produces() == Seq::new(operand_types.remaining().len(), |index: int| true),
    ;

    /// Returns the values of the type at which the behavior of an instruction can change.
    fn generate_critical_values(&self, operand_type: Self::OperandType)
    -> (critical_values: Self::CriticalValues<'_>)
        ensures
            critical_values.obeys_prophetic_iter_laws(),
            critical_values.remaining() == self.critical_values(operand_type),
            critical_values.produces() == Seq::new(critical_values.remaining().len(), |index: int| true),
    ;

    /// Returns an execution state in which the next instruction executed is the given instruction with the given operands.
    ///
    /// The operands are in the order of the instruction's signature.
    fn build_state(
        &self,
        instruction: Architecture::Instruction,
        operands: &[Self::OperandValue],
    ) -> (state: ExecutionState<'static, Architecture>)
        ensures
            state == self.state(instruction, operands@),
    ;

    /// Returns the state of every combination of the critical values of the instruction's operands.
    fn generate_critical_states(
        &self,
        instruction: Architecture::Instruction,
    ) -> (critical_states: CriticalStates<'_, Self, Architecture>)
        ensures
            critical_states.domain == self,
            critical_states.instruction == instruction,
            critical_states.combinations.obeys_prophetic_iter_laws(),
            critical_states.combinations.remaining().map_values(|operands: Vec<Self::OperandValue>| operands@)
                == product(self.operand_types(instruction).map_values(|operand_type| self.critical_values(operand_type))),
    {
        CriticalStates {
            domain: self,
            instruction,
            // VERUS: no specification of a provided method of `Itertools`, and no proof through a `map` of an iterator
            // of an associated type, so the `map` and `multi_cartesian_product` run in one trusted function.
            combinations: map_multi_cartesian_product(
                self.list_operand_types(instruction),
                |operand_type| -> (values: Vec<Self::OperandValue>)
                    ensures
                        values@ == self.critical_values(operand_type),
                {
                    self.generate_critical_values(operand_type)
                        .collect::<Vec<_>>()
                },
                Ghost(|operand_type| self.critical_values(operand_type)),
            ),
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

#[cfg(verus_keep_ghost)]
impl<Domain, Architecture> vstd::std_specs::iter::IteratorSpecImpl for CriticalStates<'_, Domain, Architecture>
where
    Domain: InstructionInputDomain<Architecture>,
    Architecture: ProcessorArchitecture,
{
    open spec fn obeys_prophetic_iter_laws(&self) -> bool {
        self.combinations.obeys_prophetic_iter_laws()
    }

    #[verifier::prophetic]
    open spec fn remaining(&self) -> Seq<ExecutionState<'static, Architecture>> {
        self.combinations.remaining().map_values(|operands: Vec<Domain::OperandValue>| self.domain.state(self.instruction, operands@))
    }

    #[verifier::prophetic]
    open spec fn produces(&self) -> Seq<bool> {
        self.combinations.produces()
    }

    open spec fn decrease(&self) -> Option<nat> {
        self.combinations.decrease()
    }

    open spec fn peek(&self, index: int) -> Option<ExecutionState<'static, Architecture>> {
        None
    }
}

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

} // verus!
