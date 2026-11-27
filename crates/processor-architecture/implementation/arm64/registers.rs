//! Reading and writing the general-purpose registers an encoding's register numbers name.

use vstd::prelude::*;

use crate::operand::RegisterThirtyOne;
use crate::{GENERAL_PURPOSE_REGISTERS, Processor};

verus! {

pub fn is_general_purpose(number: u32) -> (general_purpose: bool)
    ensures
        general_purpose == (number < GENERAL_PURPOSE_REGISTERS),
{
    (number as usize) < GENERAL_PURPOSE_REGISTERS
}

/// The value of register `number` of the processor, with 31 the zero register or the stack pointer.
pub fn read_register<LevelSpecific>(
    processor: &Processor<LevelSpecific>,
    number: u32,
    thirty_one: RegisterThirtyOne,
) -> (value: u64)
    ensures
        value == register_read(processor.general_purpose_registers@, processor.stack_pointer, number as nat, thirty_one),
{
    register_value(&processor.general_purpose_registers, processor.stack_pointer, number, thirty_one)
}

/// Writes register `number` of the processor, with 31 nothing or the stack pointer, as the operand names it; changes
/// nothing else.
pub fn write_register<LevelSpecific>(
    processor: &mut Processor<LevelSpecific>,
    number: u32,
    value: u64,
    thirty_one: RegisterThirtyOne,
)
    ensures
        final(processor).general_purpose_registers@ == register_file_written(old(processor).general_purpose_registers@, number as nat, value),
        final(processor).stack_pointer == stack_pointer_written(old(processor).stack_pointer, number as nat, value, thirty_one),
        *final(processor) == (Processor {
            general_purpose_registers: final(processor).general_purpose_registers,
            stack_pointer: final(processor).stack_pointer,
            ..*old(processor)
        }),
{
    set_register_value(
        &mut processor.general_purpose_registers,
        &mut processor.stack_pointer,
        number,
        value,
        thirty_one,
    );
}

/// X0 to X30 after a write of register `number`, which for 31 leaves them.
pub open spec fn register_file_written(registers: Seq<u64>, number: nat, value: u64) -> Seq<u64> {
    if number < registers.len() { registers.update(number as int, value) } else { registers }
}

/// The stack pointer after a write of register `number`, which only 31 naming it changes.
pub open spec fn stack_pointer_written(stack_pointer: u64, number: nat, value: u64, thirty_one: RegisterThirtyOne) -> u64 {
    if number >= GENERAL_PURPOSE_REGISTERS && thirty_one == RegisterThirtyOne::StackPointer { value } else { stack_pointer }
}

/// What a read of register `number` gives, as [X\[\]](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/aarch64-functions-registers#impl-aarch64.X.read.2)
/// and [SP\[\]](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/aarch64-functions-registers#impl-aarch64.SP.read.0)
/// read it: X0 to X30, and for 31 zero or the stack pointer, as the operand names it.
pub open spec fn register_read(registers: Seq<u64>, stack_pointer: u64, number: nat, thirty_one: RegisterThirtyOne) -> u64 {
    if number < registers.len() {
        registers[number as int]
    } else {
        match thirty_one {
            RegisterThirtyOne::ZeroRegister => 0,
            RegisterThirtyOne::StackPointer => stack_pointer,
        }
    }
}

/// The value of register `number`, with 31 the zero register or the stack pointer.
fn register_value(
    registers: &[u64; GENERAL_PURPOSE_REGISTERS],
    stack_pointer: u64,
    number: u32,
    thirty_one: RegisterThirtyOne,
) -> (value: u64)
    ensures
        value == register_read(registers@, stack_pointer, number as nat, thirty_one),
{
    // VERUS: no specification of a slice's `get`, so the index is tested.
    if (number as usize) < GENERAL_PURPOSE_REGISTERS {
        registers[number as usize]
    } else {
        match thirty_one {
            RegisterThirtyOne::ZeroRegister => 0,
            RegisterThirtyOne::StackPointer => stack_pointer,
        }
    }
}

/// Writes register `number`, with 31 the zero register, which keeps nothing, or the stack pointer, as
/// [X\[\]](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/aarch64-functions-registers#impl-aarch64.X.write.2)
/// and [SP\[\]](https://developer.arm.com/documentation/ddi0602/latest/Shared-Pseudocode/aarch64-functions-registers#impl-aarch64.SP.write.0)
/// write them.
fn set_register_value(
    registers: &mut [u64; GENERAL_PURPOSE_REGISTERS],
    stack_pointer: &mut u64,
    number: u32,
    value: u64,
    thirty_one: RegisterThirtyOne,
)
    ensures
        final(registers)@ == if number < GENERAL_PURPOSE_REGISTERS {
            old(registers)@.update(number as int, value)
        } else {
            old(registers)@
        },
        *final(stack_pointer) == if number >= GENERAL_PURPOSE_REGISTERS && thirty_one == RegisterThirtyOne::StackPointer {
            value
        } else {
            *old(stack_pointer)
        },
{
    if (number as usize) < GENERAL_PURPOSE_REGISTERS {
        registers[number as usize] = value;
    } else {
        match thirty_one {
            RegisterThirtyOne::ZeroRegister => {},
            RegisterThirtyOne::StackPointer => *stack_pointer = value,
        }
    }
}

} // verus!
