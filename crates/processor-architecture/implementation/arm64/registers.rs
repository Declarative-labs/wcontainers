//! Reading and writing the general-purpose registers an encoding's register numbers name.

use crate::operand::RegisterThirtyOne;
use crate::{GENERAL_PURPOSE_REGISTERS, Processor};

pub fn is_general_purpose(number: u32) -> bool {
    (number as usize) < GENERAL_PURPOSE_REGISTERS
}

pub fn read_register<LevelSpecific>(
    processor: &Processor<LevelSpecific>,
    number: u32,
    thirty_one: RegisterThirtyOne,
) -> u64 {
    match processor.general_purpose_registers.get(number as usize) {
        Some(value) => *value,
        None => match thirty_one {
            RegisterThirtyOne::ZeroRegister => 0,
            RegisterThirtyOne::StackPointer => processor.stack_pointer,
        },
    }
}

pub fn write_register<LevelSpecific>(
    processor: &mut Processor<LevelSpecific>,
    number: u32,
    value: u64,
    thirty_one: RegisterThirtyOne,
) {
    match processor.general_purpose_registers.get_mut(number as usize) {
        Some(register) => *register = value,
        None => match thirty_one {
            RegisterThirtyOne::ZeroRegister => {}
            RegisterThirtyOne::StackPointer => processor.stack_pointer = value,
        },
    }
}
