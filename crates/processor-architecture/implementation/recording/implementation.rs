//! What one chip did with one instruction on each state it was given.

use wcontainers_distinguishing_state_generator::DistinguishingStateGenerator;
use wcontainers_executor::Executor;
use wcontainers_processor_architecture::{
    ExecutionState, MemoryDelta, ProcessorArchitecture, Simulator,
};

#[cfg(test)]
mod tests;
mod text;

pub use text::{MalformedRecording, from_text, read_recordings, to_text, write_recordings};

/// The state before and the state the chip gave after, for each state one instruction was run on.
#[derive(Clone)]
pub struct Recording<Architecture: ProcessorArchitecture> {
    instruction: Architecture::Instruction,
    /// Each after holds its before's memory changes followed by those the chip made.
    cases: Vec<(
        ExecutionState<'static, Architecture>,
        ExecutionState<'static, Architecture>,
    )>,
}

/// The state given to a recording is none of its states before.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct NotRecorded;

impl<Architecture: ProcessorArchitecture + Clone + Eq> Recording<Architecture> {
    pub const fn instruction(&self) -> Architecture::Instruction {
        self.instruction
    }

    pub fn cases(
        &self,
    ) -> &[(
        ExecutionState<'static, Architecture>,
        ExecutionState<'static, Architecture>,
    )] {
        &self.cases
    }
}

impl<Architecture: ProcessorArchitecture + Clone + Eq> Simulator<Architecture>
    for Recording<Architecture>
{
    type Error = NotRecorded;

    /// Gives the recorded after of the recorded before that equals the processor and memory given.
    fn simulate_in_place(
        &self,
        processor: &mut Architecture::ProcessorState,
        memory: &[MemoryDelta<'_, Architecture>],
        on_memory_mutated: impl FnMut(MemoryDelta<'_, Architecture>),
    ) -> Result<(), NotRecorded> {
        let (before, after) = self
            .cases
            .iter()
            .find(|(before, _)| {
                before.processor == *processor && before.memory_mutations.as_slice() == memory
            })
            .ok_or(NotRecorded)?;
        processor.clone_from(&after.processor);
        after.memory_mutations[before.memory_mutations.len()..]
            .iter()
            .cloned()
            .for_each(on_memory_mutated);
        Ok(())
    }
}

/// Runs the instruction on the chip from every state the generator gives for it.
pub fn record<Architecture, Chip, Generator>(
    chip: &mut Chip,
    generator: &Generator,
    instruction: Architecture::Instruction,
) -> Recording<Architecture>
where
    Architecture: ProcessorArchitecture + Clone + Eq,
    Chip: Executor<Architecture = Architecture>,
    Generator: DistinguishingStateGenerator<Architecture>,
{
    let cases = generator
        .generate_states(instruction)
        .map(|before| {
            let mut after = before.clone();
            chip.execute_single_instruction_in_place(
                &mut after.processor,
                &before.memory_mutations,
                |mutation| after.memory_mutations.push(mutation.into_owned()),
            );
            (before, after)
        })
        .collect();
    Recording { instruction, cases }
}
