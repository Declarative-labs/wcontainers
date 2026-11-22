//! The precise instruction set of a chip, found by running it and comparing with the behaviors already known.

#![no_std]

extern crate alloc;

use alloc::vec::Vec;

use wcontainers_distinguishing_state_generator::DistinguishingStateGenerator;
use wcontainers_executor::Executor;
use wcontainers_precise_instruction_set::PreciseInstructionSet;
use wcontainers_processor_architecture::{
    ExecutionState, MemoryDelta, ProcessorArchitecture, Simulator,
};
use wcontainers_processor_architecture_recording::leave_same_memory;

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
    /// On this state the chip did what none of the behaviors still agreeing with it does.
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
    /// The chip agrees with each of these behaviors on every generated state.
    IndistinguishableBehaviors {
        instruction: Architecture::Instruction,
        behaviors: Vec<Architecture::PreciseInstruction>,
    },
}

impl<Architecture: ProcessorArchitecture + Clone + Eq> MeasuredInstructionSet<Architecture> {
    /// Keeps, for each instruction, the one known behavior the chip agrees with on every generated state.
    ///
    /// An instruction that every known set gives the same behavior is not run.
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

/// What one instruction did to the processor, and the memory changes it made.
struct Effect<Architecture: ProcessorArchitecture> {
    processor: Architecture::Processor,
    made: Vec<MemoryDelta<'static, Architecture>>,
}

impl<Architecture: ProcessorArchitecture + Clone + Eq> Effect<Architecture> {
    fn of(
        before: &ExecutionState<'static, Architecture>,
        run: impl FnOnce(&mut Architecture::Processor, &mut Vec<MemoryDelta<'static, Architecture>>),
    ) -> Self {
        let mut processor = before.processor.clone();
        let mut made = Vec::new();
        run(&mut processor, &mut made);
        Self { processor, made }
    }

    fn agrees_with(&self, other: &Self, before: &ExecutionState<'static, Architecture>) -> bool {
        self.processor == other.processor
            && leave_same_memory(&before.memory_mutations, &self.made, &other.made)
    }

    fn to_state(
        &self,
        before: &ExecutionState<'static, Architecture>,
    ) -> ExecutionState<'static, Architecture> {
        let mut memory_mutations = before.memory_mutations.clone();
        memory_mutations.extend(self.made.iter().cloned());
        ExecutionState {
            processor: self.processor.clone(),
            memory_mutations,
        }
    }
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
    if candidates.len() > 1 {
        for before in generator.generate_states(instruction) {
            let observed = Effect::of(&before, |processor, made| {
                chip.execute_single_instruction_in_place(
                    processor,
                    &before.memory_mutations,
                    |mutation| made.push(mutation.into_owned()),
                );
            });
            let simulated: Vec<_> = candidates
                .iter()
                .map(|precise| {
                    let effect = Effect::of(&before, |processor, made| {
                        let Ok(()) = precise.simulate_in_place(
                            processor,
                            &before.memory_mutations,
                            |mutation| made.push(mutation.into_owned()),
                        );
                    });
                    (*precise, effect)
                })
                .collect();
            let agreeing: Vec<_> = simulated
                .iter()
                .filter(|(_, effect)| effect.agrees_with(&observed, &before))
                .map(|(precise, _)| *precise)
                .collect();
            if agreeing.is_empty() {
                return Err(MeasurementFailure::NoBehaviorAgrees {
                    instruction,
                    behaviors: simulated
                        .iter()
                        .map(|(precise, effect)| (*precise, effect.to_state(&before)))
                        .collect(),
                    chip: observed.to_state(&before),
                    before,
                });
            }
            candidates = agreeing;
        }
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
