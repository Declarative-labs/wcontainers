//! Checks of the SVE models that need no chip.

use alloc::vec::Vec;

use crate::LISTED_INSTRUCTIONS;
use crate::encoding::Field;
use crate::sve::{ScalableOperand, to_scalable_instruction};

/// The fields an operand encodes.
fn to_fields(operand: ScalableOperand) -> Vec<Field> {
    match operand {
        ScalableOperand::ElementSize(field)
        | ScalableOperand::Selector(field)
        | ScalableOperand::Vector { field, .. }
        | ScalableOperand::Predicate { field, .. }
        | ScalableOperand::General { field, .. }
        | ScalableOperand::UnsignedImmediate(field)
        | ScalableOperand::SignedImmediate(field)
        | ScalableOperand::Pattern(field)
        | ScalableOperand::Bitmask(field) => [field].to_vec(),
        ScalableOperand::SplitImmediate(fields) => fields.to_vec(),
        ScalableOperand::ConditionFlags
        | ScalableOperand::WrittenConditionFlags
        | ScalableOperand::FirstFault
        | ScalableOperand::WrittenFirstFault => Vec::new(),
    }
}

/// Every field an SVE model's operands encode lies outside the instruction's fixed bits, so that each of its values
/// gives a word of the instruction, and apart from the other operands' fields.
#[test]
fn scalable_operands_lie_outside_fixed_bits() {
    for instruction in LISTED_INSTRUCTIONS {
        let Some(scalable) = to_scalable_instruction(instruction) else {
            continue;
        };
        let mut taken = instruction.to_fixed_bits().mask;
        let mut fields: Vec<Field> = Vec::new();
        for operand in scalable.operands {
            for field in to_fields(*operand) {
                // A register named twice, as a destination that is also a source, encodes one field.
                if fields.contains(&field) {
                    continue;
                }
                assert_eq!(
                    field.to_mask() & taken,
                    0,
                    "{instruction:?}: {operand:?} overlaps the fixed bits or another operand"
                );
                taken |= field.to_mask();
                fields.push(field);
            }
        }
    }
}
