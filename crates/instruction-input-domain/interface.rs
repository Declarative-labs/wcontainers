#![no_std]

extern crate alloc;

use alloc::vec::Vec;

use itertools::Itertools;
use wcontainers_processor_architecture::{ExecutionState, ProcessorArchitecture};

pub trait InstructionInputDomain<Architecture: ProcessorArchitecture> {
    /// The type of one input of an instruction, which does not depend on the types of its other inputs.
    type OperandType: Copy + Eq;
    type OperandValue: Copy;

    /// Returns the type of each of the instruction's operands, in the order of its signature.
    fn list_operand_types(
        &self,
        instruction: Architecture::Instruction,
    ) -> impl Iterator<Item = Self::OperandType>;

    /// Returns the values of the type at which the behavior of an instruction can change.
    fn generate_critical_values(
        &self,
        operand_type: Self::OperandType,
    ) -> impl Iterator<Item = Self::OperandValue>;

    /// Returns an execution state in which the next instruction executed is the given instruction with the given operands.
    ///
    /// The operands are in the order of the instruction's signature.
    fn build_state(
        &self,
        instruction: Architecture::Instruction,
        operands: &[Self::OperandValue],
    ) -> ExecutionState<'static, Architecture>;

    /// Returns the state of every combination of the critical values of the instruction's operands.
    fn generate_critical_states(
        &self,
        instruction: Architecture::Instruction,
    ) -> impl Iterator<Item = ExecutionState<'static, Architecture>> {
        self.list_operand_types(instruction)
            .map(|operand_type| {
                self.generate_critical_values(operand_type)
                    .collect::<Vec<_>>()
            })
            .multi_cartesian_product()
            .map(move |operands| self.build_state(instruction, &operands))
    }
}
