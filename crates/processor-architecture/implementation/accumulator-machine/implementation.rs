//! A machine of one 8-bit accumulator and a carry flag, whose instructions are one byte each.
//!
//! Its four instructions have one, two or four known behaviors, so that what is written about an architecture can be
//! read whole and checked against every case a real one has: one behavior, several, and one that holds fields.

#![no_std]

extern crate alloc;

use alloc::vec::{self, Vec};
use core::convert::Infallible;
use core::ops::Range;

use wcontainers_processor_architecture::{
    MemorySink, MemorySnapshot, ProcessorArchitecture, Simulator,
};

/// The bytes of one instruction, its opcode.
pub const INSTRUCTION_SIZE: u16 = size_of::<u8>() as u16;

/// Every instruction, in the order of their opcodes.
pub const INSTRUCTIONS: &[Instruction] = &[
    Instruction::NoOperation,
    Instruction::ClearAccumulator,
    Instruction::Increment,
    Instruction::ShiftLeft,
];

/// A machine whose program counter addresses 64 KiB of memory.
#[derive(Clone, Default, PartialEq, Eq, Debug)]
pub struct AccumulatorMachineProcessorArchitecture;

impl ProcessorArchitecture for AccumulatorMachineProcessorArchitecture {
    type ProcessorState = Processor;
    /// A behavior decides every value of the processor.
    type ProcessorStateExpectation = Processor;
    type PagePermission = PagePermission;
    type Instruction = Instruction;
    type PreciseInstruction = PreciseInstruction;
    type Instructions = vec::IntoIter<Instruction>;
    type KnownBehaviors = vec::IntoIter<PreciseInstruction>;

    const ADDRESSABLE: Range<u64> = 0..1 << u16::BITS;
    const DEFAULT_PERMISSION: PagePermission = PagePermission::Unmapped;

    fn list_instructions() -> vec::IntoIter<Instruction> {
        INSTRUCTIONS.to_vec().into_iter()
    }

    fn list_known_behaviors(instruction: Instruction) -> vec::IntoIter<PreciseInstruction> {
        let behaviors = match instruction {
            Instruction::NoOperation => Vec::from([PreciseInstruction::NoOperation]),
            Instruction::ClearAccumulator => Vec::from([PreciseInstruction::ClearAccumulator]),
            Instruction::Increment => Vec::from([
                PreciseInstruction::Increment(IncrementOverflow::Wraps),
                PreciseInstruction::Increment(IncrementOverflow::Saturates),
            ]),
            Instruction::ShiftLeft => [
                CarryAfterShift::LastBitShiftedOut,
                CarryAfterShift::Unchanged,
            ]
            .into_iter()
            .flat_map(|carry| {
                [LowBitAfterShift::Zero, LowBitAfterShift::Carry]
                    .map(|low_bit| PreciseInstruction::ShiftLeft { carry, low_bit })
            })
            .collect(),
        };
        behaviors.into_iter()
    }

    fn meets_expectations(observed: &Processor, expectation: &Processor) -> bool {
        observed == expectation
    }
}

/// The registers of the machine.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Processor {
    pub accumulator: u8,
    pub carry: bool,
    pub program_counter: u16,
}

/// What the running program may do with a page; memory that was never given a permission is unmapped.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PagePermission {
    Unmapped,
    Executable,
}

/// An instruction, whose opcode is its place in [`INSTRUCTIONS`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(
    feature = "bake",
    derive(databake::Bake),
    databake(path = wcontainers_processor_architecture_accumulator_machine)
)]
pub enum Instruction {
    NoOperation,
    ClearAccumulator,
    /// Adds one to the accumulator.
    Increment,
    /// Shifts the accumulator left by one bit.
    ShiftLeft,
}

/// An instruction with one behavior for every case a core of the machine may do its own way.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(
    feature = "bake",
    derive(databake::Bake),
    databake(path = wcontainers_processor_architecture_accumulator_machine)
)]
pub enum PreciseInstruction {
    NoOperation,
    ClearAccumulator,
    Increment(IncrementOverflow),
    ShiftLeft {
        carry: CarryAfterShift,
        low_bit: LowBitAfterShift,
    },
}

/// What an increment of the largest accumulator leaves.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(
    feature = "bake",
    derive(databake::Bake),
    databake(path = wcontainers_processor_architecture_accumulator_machine)
)]
pub enum IncrementOverflow {
    /// Zero.
    Wraps,
    /// The largest accumulator.
    Saturates,
}

/// What a shift leaves in the carry flag.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(
    feature = "bake",
    derive(databake::Bake),
    databake(path = wcontainers_processor_architecture_accumulator_machine)
)]
pub enum CarryAfterShift {
    LastBitShiftedOut,
    Unchanged,
}

/// What a shift moves into the lowest bit of the accumulator.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(
    feature = "bake",
    derive(databake::Bake),
    databake(path = wcontainers_processor_architecture_accumulator_machine)
)]
pub enum LowBitAfterShift {
    Zero,
    /// The carry flag from before the shift, as a rotate through the carry does.
    Carry,
}

/// What a core of the machine says it is.
#[derive(Clone, PartialEq, Eq, Debug)]
#[cfg_attr(
    feature = "bake",
    derive(databake::Bake),
    databake(path = wcontainers_processor_architecture_accumulator_machine)
)]
pub struct CoreIdentity {
    pub design: CoreDesign,
    pub revision: CoreRevision,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(
    feature = "bake",
    derive(databake::Bake),
    databake(path = wcontainers_processor_architecture_accumulator_machine)
)]
pub enum CoreDesign {
    /// The design this crate describes.
    Reference,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(
    feature = "bake",
    derive(databake::Bake),
    databake(path = wcontainers_processor_architecture_accumulator_machine)
)]
pub enum CoreRevision {
    First,
    Second,
}

impl PreciseInstruction {
    pub const fn to_instruction(self) -> Instruction {
        match self {
            Self::NoOperation => Instruction::NoOperation,
            Self::ClearAccumulator => Instruction::ClearAccumulator,
            Self::Increment(_) => Instruction::Increment,
            Self::ShiftLeft { .. } => Instruction::ShiftLeft,
        }
    }
}

impl Simulator<AccumulatorMachineProcessorArchitecture> for PreciseInstruction {
    type Error = Infallible;

    /// Changes no memory.
    fn simulate(
        &self,
        processor: &Processor,
        _: MemorySnapshot<'_, '_, AccumulatorMachineProcessorArchitecture>,
        _: &mut impl MemorySink<AccumulatorMachineProcessorArchitecture>,
    ) -> Result<Processor, Infallible> {
        let mut after = processor.clone();
        after.program_counter = processor.program_counter.wrapping_add(INSTRUCTION_SIZE);
        match *self {
            Self::NoOperation => {}
            Self::ClearAccumulator => after.accumulator = u8::MIN,
            Self::Increment(IncrementOverflow::Wraps) => {
                after.accumulator = processor.accumulator.wrapping_add(1);
            }
            Self::Increment(IncrementOverflow::Saturates) => {
                after.accumulator = processor.accumulator.saturating_add(1);
            }
            Self::ShiftLeft { carry, low_bit } => {
                let low_bit = match low_bit {
                    LowBitAfterShift::Zero => u8::MIN,
                    LowBitAfterShift::Carry => u8::from(processor.carry),
                };
                after.accumulator = processor.accumulator << 1 | low_bit;
                if carry == CarryAfterShift::LastBitShiftedOut {
                    after.carry = processor.accumulator.leading_zeros() == u32::MIN;
                }
            }
        }
        Ok(after)
    }
}
