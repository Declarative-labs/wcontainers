//! The inputs of the accumulator machine's instructions: the accumulator and the carry flag.
//!
//! Each state holds the instruction's opcode at the program counter, on an executable page of its own.

#![no_std]

extern crate alloc;

use alloc::borrow::Cow;
use alloc::vec::{self, Vec};

use wcontainers_instruction_input_domain::InstructionInputDomain;
use wcontainers_processor_architecture::{ExecutionState, MemoryDelta};
use wcontainers_processor_architecture_accumulator_machine::{
    AccumulatorMachineProcessorArchitecture, INSTRUCTION_SIZE, INSTRUCTIONS, Instruction,
    PagePermission, Processor,
};

/// The values at which an instruction's behavior can change: zero, the largest, and the two either side of the top bit.
const CRITICAL_ACCUMULATORS: &[u8] = &[
    u8::MIN,
    i8::MAX.cast_unsigned(),
    i8::MIN.cast_unsigned(),
    u8::MAX,
];
const CRITICAL_CARRIES: &[bool] = &[false, true];

#[derive(Clone, Copy, Default, Debug)]
pub struct AccumulatorMachineInstructionInputDomain;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OperandType {
    Accumulator,
    Carry,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OperandValue {
    Accumulator(u8),
    Carry(bool),
}

impl InstructionInputDomain<AccumulatorMachineProcessorArchitecture>
    for AccumulatorMachineInstructionInputDomain
{
    type OperandType = OperandType;
    type OperandValue = OperandValue;
    type OperandTypes<'domain> = vec::IntoIter<OperandType>;
    type CriticalValues<'domain> = vec::IntoIter<OperandValue>;

    /// Lists the accumulator and the carry flag, which every instruction reads.
    fn list_operand_types(&self, _: Instruction) -> vec::IntoIter<OperandType> {
        Vec::from([OperandType::Accumulator, OperandType::Carry]).into_iter()
    }

    fn generate_critical_values(&self, operand_type: OperandType) -> vec::IntoIter<OperandValue> {
        let values: Vec<OperandValue> = match operand_type {
            OperandType::Accumulator => CRITICAL_ACCUMULATORS
                .iter()
                .copied()
                .map(OperandValue::Accumulator)
                .collect(),
            OperandType::Carry => CRITICAL_CARRIES
                .iter()
                .copied()
                .map(OperandValue::Carry)
                .collect(),
        };
        values.into_iter()
    }

    /// Puts the instruction at address zero.
    fn build_state(
        &self,
        instruction: Instruction,
        operands: &[OperandValue],
    ) -> ExecutionState<'static, AccumulatorMachineProcessorArchitecture> {
        let mut processor = Processor {
            accumulator: u8::MIN,
            carry: false,
            program_counter: u16::MIN,
        };
        for operand in operands {
            match *operand {
                OperandValue::Accumulator(accumulator) => processor.accumulator = accumulator,
                OperandValue::Carry(carry) => processor.carry = carry,
            }
        }
        let Some(opcode) = INSTRUCTIONS
            .iter()
            .position(|listed| *listed == instruction)
        else {
            unreachable!("every instruction is listed");
        };
        let start = u64::from(processor.program_counter);
        ExecutionState {
            processor,
            memory_mutations: Vec::from([
                MemoryDelta::PermissionAssignment {
                    addresses: start..start + u64::from(INSTRUCTION_SIZE),
                    permission: PagePermission::Executable,
                },
                MemoryDelta::ValueAssignment {
                    start,
                    bytes: Cow::Owned(Vec::from([opcode as u8])),
                },
            ]),
        }
    }
}
