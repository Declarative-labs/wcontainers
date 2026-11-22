//! A generator that forms every combination of each operand's critical values.

#![no_std]

extern crate alloc;

use alloc::vec::Vec;

use wcontainers_distinguishing_state_generator::DistinguishingStateGenerator;
use wcontainers_processor_architecture::{ExecutionState, ProcessorArchitecture};

mod arm64;
#[cfg(test)]
mod tests;

pub use arm64::{AccessPlacement, Arm64Operand, Arm64OperandValue, DataPage};

/// The inputs of an architecture's instructions.
///
/// TODO: temporary placeholder until x86 confirms its shape and it moves into an interface.
pub trait InstructionInputDomain: ProcessorArchitecture {
    /// An input of an instruction whose type does not depend on the instruction's other inputs; its values may.
    type Operand: Copy + Eq;
    type OperandValue: Copy;

    /// The order means nothing.
    fn list_operands(instruction: Self::Instruction) -> impl Iterator<Item = Self::Operand>;

    /// The critical values of the operand: zero, one, the largest, the sign bit alone, the same register twice, and so on.
    fn generate_values(operand: Self::Operand) -> impl Iterator<Item = Self::OperandValue>;

    /// The state about to run the instruction with one value for each operand, in the order `list_operands` gives.
    fn to_state(
        instruction: Self::Instruction,
        values: &[Self::OperandValue],
    ) -> ExecutionState<'static, Self>;
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct CriticalValueProduct;

impl<Architecture: InstructionInputDomain> DistinguishingStateGenerator<Architecture>
    for CriticalValueProduct
{
    fn generate_states(
        &self,
        instruction: Architecture::Instruction,
    ) -> impl Iterator<Item = ExecutionState<'static, Architecture>> {
        let value_lists: Vec<Vec<Architecture::OperandValue>> =
            Architecture::list_operands(instruction)
                .map(|operand| Architecture::generate_values(operand).collect())
                .collect();
        let combinations = value_lists.iter().map(Vec::len).product();
        // Each combination is a number whose digits, in mixed radix, pick one value from each list.
        (0..combinations).map(move |combination: usize| {
            let mut rest = combination;
            let values: Vec<Architecture::OperandValue> = value_lists
                .iter()
                .map(|values| {
                    let value = values[rest % values.len()];
                    rest /= values.len();
                    value
                })
                .collect();
            Architecture::to_state(instruction, &values)
        })
    }
}
