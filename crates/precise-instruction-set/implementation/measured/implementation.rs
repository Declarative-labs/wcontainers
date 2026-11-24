//! The precise instruction set of a chip, found by running it and comparing with the behaviors already known.

#![no_std]

extern crate alloc;

use alloc::vec::Vec;
use core::fmt::{self, Debug, Formatter};

use wcontainers_distinguishing_state_generator::DistinguishingStateGenerator;
use wcontainers_executor::Executor;
use wcontainers_precise_instruction_set::PreciseInstructionSet;
use wcontainers_processor_architecture::{
    ExecutionState, MemoryDelta, ProcessorArchitecture, Simulator,
};
use wcontainers_processor_architecture_conformance::leave_same_memory;

#[cfg(test)]
mod tests;

pub struct MeasuredInstructionSet<Architecture: ProcessorArchitecture> {
    /// Every listed instruction, with the behavior the chip showed.
    behaviors: Vec<(Architecture::Instruction, Architecture::PreciseInstruction)>,
}

/// Why the chip's behavior of an instruction is not one known behavior.
pub enum MeasurementFailure<Architecture: ProcessorArchitecture> {
    NoKnownBehavior {
        instruction: Architecture::Instruction,
    },
    /// On this state the chip did what none of the behaviors still in play does.
    NoBehaviorAgrees {
        instruction: Architecture::Instruction,
        before: ExecutionState<'static, Architecture>,
        /// Each behavior with the state after it simulates.
        behaviors: Vec<(
            Architecture::PreciseInstruction,
            ExecutionState<'static, Architecture>,
        )>,
        chip: ExecutionState<'static, Architecture>,
    },
    /// No generated state tells these behaviors apart.
    IndistinguishableBehaviors {
        instruction: Architecture::Instruction,
        behaviors: Vec<Architecture::PreciseInstruction>,
    },
}

impl<Architecture: ProcessorArchitecture + Clone + Eq> MeasuredInstructionSet<Architecture> {
    /// Keeps, for each instruction, the one known behavior the chip agrees with.
    ///
    /// The chip runs only the generated states on which the behaviors still in play differ, so an instruction every
    /// known set gives the same behavior is not run. That the kept behavior agrees with the chip on every state is
    /// what the chip's recordings check, once.
    ///
    /// TODO: temporary placeholder until the interface says how a precise instruction set is built.
    pub fn measure<Chip, Generator>(
        chip: &mut Chip,
        known: &[&dyn PreciseInstructionSet<Architecture>],
        generator: &Generator,
    ) -> Result<Self, MeasurementFailure<Architecture>>
    where
        Chip: Executor<Architecture = Architecture>,
        Generator: DistinguishingStateGenerator<Architecture>,
    {
        let behaviors = Architecture::list_instructions()
            .map(|instruction| {
                let mut candidates = Vec::new();
                for set in known {
                    let precise = set.to_precise(instruction);
                    if !candidates.contains(&precise) {
                        candidates.push(precise);
                    }
                }
                let precise = measure_instruction(chip, generator, instruction, candidates)?;
                Ok((instruction, precise))
            })
            .collect::<Result<_, _>>()?;
        Ok(Self { behaviors })
    }
}

impl<Architecture: ProcessorArchitecture> PreciseInstructionSet<Architecture>
    for MeasuredInstructionSet<Architecture>
{
    /// Panics for an instruction that `list_instructions` does not list.
    fn to_precise(
        &self,
        instruction: Architecture::Instruction,
    ) -> Architecture::PreciseInstruction {
        let measured = self
            .behaviors
            .iter()
            .find(|(measured, _)| *measured == instruction);
        match measured {
            Some((_, precise)) => *precise,
            None => panic!("only listed instructions are measured"),
        }
    }
}

impl<Architecture> Debug for MeasurementFailure<Architecture>
where
    Architecture: ProcessorArchitecture,
    Architecture::Instruction: Debug,
    Architecture::PreciseInstruction: Debug,
    Architecture::ProcessorState: Debug,
    Architecture::PagePermission: Debug,
{
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoKnownBehavior { instruction } => {
                write!(formatter, "no known set gives {instruction:?} a behavior")
            }
            Self::NoBehaviorAgrees {
                instruction,
                before,
                behaviors,
                chip,
            } => {
                let start = before.memory_mutations.len();
                writeln!(
                    formatter,
                    "no known behavior of {instruction:?} agrees with the chip"
                )?;
                writeln!(formatter, "  before: ")?;
                write_state(formatter, &before.processor, &before.memory_mutations)?;
                for (precise, after) in behaviors {
                    writeln!(formatter, "  {precise:?} gives: ")?;
                    write_state(
                        formatter,
                        &after.processor,
                        &after.memory_mutations[start..],
                    )?;
                }
                writeln!(formatter, "  the chip gave: ")?;
                write_state(formatter, &chip.processor, &chip.memory_mutations[start..])
            }
            Self::IndistinguishableBehaviors {
                instruction,
                behaviors,
            } => write!(
                formatter,
                "no generated state of {instruction:?} tells these behaviors apart: {behaviors:?}"
            ),
        }
    }
}

fn write_state<Architecture>(
    formatter: &mut Formatter<'_>,
    processor: &Architecture::ProcessorState,
    memory: &[MemoryDelta<'_, Architecture>],
) -> fmt::Result
where
    Architecture: ProcessorArchitecture,
    Architecture::ProcessorState: Debug,
    Architecture::PagePermission: Debug,
{
    writeln!(formatter, "    {processor:?}")?;
    for delta in memory {
        match delta {
            MemoryDelta::PermissionAssignment {
                addresses,
                permission,
            } => writeln!(formatter, "    {addresses:#x?} becomes {permission:?}")?,
            MemoryDelta::ValueAssignment { start, bytes } => {
                writeln!(formatter, "    {start:#x} gets {bytes:02x?}")?
            }
        }
    }
    Ok(())
}

/// Whether two states after the same state before hold the same processor and memory.
fn agree<Architecture: ProcessorArchitecture>(
    before: &ExecutionState<'_, Architecture>,
    first: &ExecutionState<'_, Architecture>,
    second: &ExecutionState<'_, Architecture>,
) -> bool {
    let start = before.memory_mutations.len();
    first.processor == second.processor
        && leave_same_memory(
            &before.memory_mutations,
            &first.memory_mutations[start..],
            &second.memory_mutations[start..],
        )
}

fn measure_instruction<Architecture, Chip, Generator>(
    chip: &mut Chip,
    generator: &Generator,
    instruction: Architecture::Instruction,
    mut candidates: Vec<Architecture::PreciseInstruction>,
) -> Result<Architecture::PreciseInstruction, MeasurementFailure<Architecture>>
where
    Architecture: ProcessorArchitecture + Clone + Eq,
    Chip: Executor<Architecture = Architecture>,
    Generator: DistinguishingStateGenerator<Architecture>,
{
    let mut states = generator.generate_states(instruction);
    while candidates.len() > 1
        && let Some(before) = states.next()
    {
        let simulated: Vec<_> = candidates
            .iter()
            .map(|precise| {
                let Ok(after) = precise.simulate(before.clone());
                (*precise, after)
            })
            .collect();
        let (_, first) = &simulated[0];
        if simulated
            .iter()
            .all(|(_, after)| agree(&before, after, first))
        {
            continue;
        }
        let executed = chip.execute_single_instruction(before.clone());
        let agreeing: Vec<_> = simulated
            .iter()
            .filter(|(_, after)| agree(&before, after, &executed))
            .map(|(precise, _)| *precise)
            .collect();
        if agreeing.is_empty() {
            return Err(MeasurementFailure::NoBehaviorAgrees {
                instruction,
                before,
                behaviors: simulated,
                chip: executed,
            });
        }
        candidates = agreeing;
    }
    match candidates.as_slice() {
        [] => Err(MeasurementFailure::NoKnownBehavior { instruction }),
        [precise] => Ok(*precise),
        [_, _, ..] => Err(MeasurementFailure::IndistinguishableBehaviors {
            instruction,
            behaviors: candidates,
        }),
    }
}
