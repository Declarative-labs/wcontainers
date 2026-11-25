//! What one ARM64 chip did with each instruction on each state it was given, played back as the chip.

use std::convert::Infallible;
use std::fmt::Debug;
use std::io;
use std::path::Path;

use wcontainers_behavior_source::BehaviorSource;
use wcontainers_distinguishing_state_generator::DistinguishingStateGenerator;
use wcontainers_processor_architecture::{
    ExecutionState, MemoryDelta, MemorySink, ProcessorArchitecture,
};
use wcontainers_processor_architecture_arm64::{Arm64, Instruction, Processor, User};

#[cfg(test)]
mod tests;
mod text;

/// One state before and the state the chip gave after, which holds its before's memory mutations followed by the chip's.
type RecordedCase = (
    ExecutionState<'static, Arm64<User>>,
    ExecutionState<'static, Arm64<User>>,
);

/// Each listed instruction with every state it was run on and what the chip gave, in the order they were run.
pub struct Recording {
    instructions: Vec<(Instruction<Infallible>, Vec<RecordedCase>)>,
    /// Where the next state is looked for first: the instruction and the case after the one played last.
    next: (usize, usize),
    /// The instruction and the case played last, where the next state is looked for second.
    played: Option<(usize, usize)>,
}

impl Recording {
    /// Runs each of the instructions on the source from every state the generator gives for it.
    pub fn record(
        source: &mut impl BehaviorSource<Architecture = Arm64<User>>,
        generator: &impl DistinguishingStateGenerator<Arm64<User>>,
        instructions: impl IntoIterator<Item = Instruction<Infallible>>,
    ) -> Recording {
        let instructions = instructions
            .into_iter()
            .map(|instruction| {
                let cases = generator
                    .generate_states(instruction)
                    .map(|before| run_case(source, before))
                    .collect();
                (instruction, cases)
            })
            .collect();
        Recording {
            instructions,
            next: (0, 0),
            played: None,
        }
    }

    /// Runs every state this recording was made from on the source again, in the same order.
    pub fn record_again(
        &self,
        source: &mut impl BehaviorSource<Architecture = Arm64<User>>,
    ) -> Recording {
        let instructions = self
            .instructions
            .iter()
            .map(|(instruction, cases)| {
                let cases = cases
                    .iter()
                    .map(|(before, _)| run_case(source, before.clone()))
                    .collect();
                (*instruction, cases)
            })
            .collect();
        Recording {
            instructions,
            next: (0, 0),
            played: None,
        }
    }

    /// Reads the recording of each listed instruction that has a file in the folder `write` wrote.
    ///
    /// A listed instruction with no file is not in the recording, which panics on any state of it, as on any other
    /// state it holds no case for.
    pub fn read(folder: &Path) -> io::Result<Recording> {
        let mut instructions = Vec::new();
        for instruction in Arm64::<User>::list_instructions() {
            let path = folder.join(text::to_file_name(instruction));
            let malformed = |what: &dyn Debug| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("{}: {what:?}", path.display()),
                )
            };
            let text = match std::fs::read_to_string(&path) {
                Ok(text) => text,
                Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
                Err(error) => return Err(error),
            };
            let (named, cases) = text::from_text(&text).map_err(|error| malformed(&error))?;
            if named != instruction {
                return Err(malformed(&named));
            }
            instructions.push((instruction, cases));
        }
        Ok(Recording {
            instructions,
            next: (0, 0),
            played: None,
        })
    }

    /// Returns each instruction the recording holds, in its order.
    pub fn instructions(&self) -> impl Iterator<Item = Instruction<Infallible>> {
        self.instructions
            .iter()
            .map(|(instruction, _)| *instruction)
    }

    /// Writes each instruction's recording into the file named after it in the folder.
    pub fn write(&self, folder: &Path) -> io::Result<()> {
        for (instruction, cases) in &self.instructions {
            std::fs::write(
                folder.join(text::to_file_name(*instruction)),
                text::to_text(*instruction, cases),
            )?;
        }
        Ok(())
    }

    /// Returns each recorded instruction with a state before and the state the chip gave after.
    pub fn cases(
        &self,
    ) -> impl Iterator<
        Item = (
            Instruction<Infallible>,
            &ExecutionState<'static, Arm64<User>>,
            &ExecutionState<'static, Arm64<User>>,
        ),
    > {
        self.instructions.iter().flat_map(|(instruction, cases)| {
            cases
                .iter()
                .map(move |(before, after)| (*instruction, before, after))
        })
    }

    /// Returns where the case recorded from exactly this processor and memory lies, looking first at the case after
    /// the one played last and at that one.
    fn find_case(
        &self,
        processor: &Processor<()>,
        memory: &[MemoryDelta<'_, Arm64<User>>],
    ) -> Option<(usize, usize)> {
        let is_recorded_from = |(before, _): &RecordedCase| {
            before.processor == *processor && before.memory_mutations.as_slice() == memory
        };
        let is_at = |&(instruction, case): &(usize, usize)| {
            self.instructions
                .get(instruction)
                .and_then(|(_, cases)| cases.get(case))
                .is_some_and(is_recorded_from)
        };
        let likely = [Some(self.next), self.played];
        if let Some(found) = likely.into_iter().flatten().find(is_at) {
            return Some(found);
        }
        self.instructions
            .iter()
            .enumerate()
            .find_map(|(instruction, (_, cases))| {
                let case = cases.iter().position(is_recorded_from)?;
                Some((instruction, case))
            })
    }
}

/// Runs one state on the source, giving the state before and the state after.
fn run_case(
    source: &mut impl BehaviorSource<Architecture = Arm64<User>>,
    before: ExecutionState<'static, Arm64<User>>,
) -> RecordedCase {
    let mut after = before.clone();
    source.execute_single_instruction_in_place(
        &mut after.processor,
        &before.memory_mutations,
        &mut after.memory_mutations,
    );
    (before, after)
}

impl BehaviorSource for Recording {
    type Architecture = Arm64<User>;

    /// Gives what the chip did from the recorded state equal to the processor and memory, and panics where none is.
    fn execute_single_instruction_in_place(
        &mut self,
        processor: &mut Processor<()>,
        memory: &[MemoryDelta<'_, Arm64<User>>],
        mutation_hook: &mut impl MemorySink<Arm64<User>>,
    ) {
        let Some((instruction, case)) = self.find_case(processor, memory) else {
            panic!("the recording holds no state equal to the one given");
        };
        let cases = &self.instructions[instruction].1;
        let (before, after) = &cases[case];
        processor.clone_from(&after.processor);
        for mutation in &after.memory_mutations[before.memory_mutations.len()..] {
            mutation_hook.on_memory_mutated(mutation.clone());
        }
        self.played = Some((instruction, case));
        self.next = if case + 1 < cases.len() {
            (instruction, case + 1)
        } else {
            (instruction + 1, 0)
        };
    }
}
