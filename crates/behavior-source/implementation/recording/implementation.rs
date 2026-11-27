//! What one ARM64 chip did with each instruction on each state it was given, played back as the chip.

use std::cell::OnceCell;
use std::convert::Infallible;
use std::fmt::Debug;
use std::io;
use std::path::Path;

use wcontainers_behavior_source::BehaviorSource;
use wcontainers_distinguishing_state_generator::DistinguishingStateGenerator;
use wcontainers_processor_architecture::{
    ExecutionState, MemoryDelta, MemorySink, MemorySnapshot, ProcessorArchitecture,
};
use wcontainers_processor_architecture_arm64::{Arm64, Instruction, Processor, User};

pub use crate::deciding::WrittenStates;
use crate::generated::{GeneratedCases, to_fingerprint};

mod deciding;
mod generated;
#[cfg(test)]
mod tests;
mod text;

/// One state before and the state the chip gave after, which holds its before's memory mutations followed by the chip's.
type RecordedCase = (
    ExecutionState<'static, Arm64<User>>,
    ExecutionState<'static, Arm64<User>>,
);

/// The state given to a recording is none of its states before.
#[derive(Debug)]
pub struct NotRecorded;

/// The cases of one instruction.
enum Cases {
    /// Each state before as it was written, with the state the chip gave after.
    Written(Vec<RecordedCase>),
    /// What the chip gave after each state a generator gives, the states before held as fingerprints.
    Generated(GeneratedCases),
}

impl Cases {
    fn len(&self) -> usize {
        match self {
            Cases::Written(cases) => cases.len(),
            Cases::Generated(cases) => cases.len(),
        }
    }
}

/// Each listed instruction with every state it was run on and what the chip gave, in the order they were run.
pub struct Recording {
    instructions: Vec<(Instruction<Infallible>, Cases)>,
    /// Where the next state is looked for first: the instruction and the case after the one played last.
    next: (usize, usize),
    /// The instruction and the case played last, where the next state is looked for second.
    played: Option<(usize, usize)>,
}

impl Recording {
    fn new(instructions: Vec<(Instruction<Infallible>, Cases)>) -> Self {
        Recording {
            instructions,
            next: (0, 0),
            played: None,
        }
    }

    /// Runs each of the instructions on the source from every state the generator gives for it, up to the first error.
    ///
    /// The recording keeps no state before, so reading it back needs the same generator.
    pub fn record<Source: BehaviorSource<Architecture = Arm64<User>>>(
        source: &mut Source,
        generator: &impl DistinguishingStateGenerator<Arm64<User>>,
        instructions: impl IntoIterator<Item = Instruction<Infallible>>,
    ) -> Result<Recording, Source::Error> {
        let instructions = instructions
            .into_iter()
            .map(|instruction| {
                let cases = GeneratedCases::record(source, generator, instruction)?;
                Ok((instruction, Cases::Generated(cases)))
            })
            .collect::<Result<_, _>>()?;
        Ok(Recording::new(instructions))
    }

    /// Runs every state this recording was made from on the source again, in the same order, up to the first error.
    ///
    /// The states of an instruction recorded from a generator are the ones the generator gives now.
    pub fn record_again<Source: BehaviorSource<Architecture = Arm64<User>>>(
        &self,
        source: &mut Source,
        generator: &impl DistinguishingStateGenerator<Arm64<User>>,
    ) -> Result<Recording, Source::Error> {
        let instructions = self
            .instructions
            .iter()
            .map(|(instruction, cases)| {
                let cases = match cases {
                    Cases::Written(cases) => Cases::Written(
                        cases
                            .iter()
                            .map(|(before, _)| run_case(source, before.clone()))
                            .collect::<Result<_, _>>()?,
                    ),
                    Cases::Generated(_) => {
                        Cases::Generated(GeneratedCases::record(source, generator, *instruction)?)
                    }
                };
                Ok((*instruction, cases))
            })
            .collect::<Result<_, _>>()?;
        Ok(Recording::new(instructions))
    }

    /// Reads the recording of each listed instruction that has a file in the folder `write` wrote, where each file
    /// writes its states before.
    ///
    /// A listed instruction with no file is not in the recording, which gives `NotRecorded` on any state of it, as on
    /// any other state it holds no case for.
    pub fn read(folder: &Path) -> io::Result<Recording> {
        Self::read_files(folder, |path, text| {
            if text::is_generated_text(text) {
                return Err(to_invalid_data(
                    path,
                    &"made from a generator, so read with read_generated",
                ));
            }
            let (instruction, cases) =
                text::from_text(text).map_err(|error| to_invalid_data(path, &error))?;
            Ok((instruction, Cases::Written(cases)))
        })
    }

    /// Reads the folder as `read` does, also each file made from a generator's states, which must be this generator's.
    pub fn read_generated(
        folder: &Path,
        generator: &impl DistinguishingStateGenerator<Arm64<User>>,
    ) -> io::Result<Recording> {
        Self::read_files(folder, |path, text| {
            if !text::is_generated_text(text) {
                let (instruction, cases) =
                    text::from_text(text).map_err(|error| to_invalid_data(path, &error))?;
                return Ok((instruction, Cases::Written(cases)));
            }
            let (instruction, cases) = GeneratedCases::from_text(text, generator)
                .map_err(|error| to_invalid_data(path, &error.to_string()))?;
            Ok((instruction, Cases::Generated(cases)))
        })
    }

    /// Reads the file of each listed instruction that has one, in the listed order; a missing folder is an error.
    fn read_files(
        folder: &Path,
        read_file: impl Fn(&Path, &str) -> io::Result<(Instruction<Infallible>, Cases)>,
    ) -> io::Result<Recording> {
        std::fs::read_dir(folder)?;
        let mut instructions = Vec::new();
        for instruction in Arm64::<User>::list_instructions() {
            let path = folder.join(text::to_file_name(instruction));
            let text = match std::fs::read_to_string(&path) {
                Ok(text) => text,
                Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
                Err(error) => return Err(error),
            };
            let (named, cases) = read_file(&path, &text)?;
            if named != instruction {
                return Err(to_invalid_data(&path, &named));
            }
            instructions.push((instruction, cases));
        }
        Ok(Recording::new(instructions))
    }

    /// Returns each instruction the recording holds, in its order.
    pub fn instructions(&self) -> impl Iterator<Item = Instruction<Infallible>> {
        self.instructions
            .iter()
            .map(|(instruction, _)| *instruction)
    }

    /// Returns how many states the recording holds.
    pub fn count_cases(&self) -> usize {
        self.instructions.iter().map(|(_, cases)| cases.len()).sum()
    }

    /// Writes each instruction's recording into the file named after it in the folder.
    pub fn write(&self, folder: &Path) -> io::Result<()> {
        for (instruction, cases) in &self.instructions {
            let text = match cases {
                Cases::Written(cases) => text::to_text(*instruction, cases),
                Cases::Generated(cases) => cases.to_text(*instruction),
            };
            std::fs::write(folder.join(text::to_file_name(*instruction)), text)?;
        }
        Ok(())
    }

    /// Returns each instruction with a state before and the state the chip gave after, of the instructions whose
    /// states before are written; `generate_cases` also gives those recorded from a generator.
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
            match cases {
                Cases::Written(cases) => cases.as_slice(),
                Cases::Generated(_) => &[],
            }
            .iter()
            .map(move |(before, after)| (*instruction, before, after))
        })
    }

    /// Returns each recorded instruction with a state before and the state the chip gave after, the states of an
    /// instruction recorded from a generator made again by this one.
    pub fn generate_cases<'cases>(
        &'cases self,
        generator: &'cases impl DistinguishingStateGenerator<Arm64<User>>,
    ) -> impl Iterator<
        Item = (
            Instruction<Infallible>,
            ExecutionState<'static, Arm64<User>>,
            ExecutionState<'static, Arm64<User>>,
        ),
    > + 'cases {
        self.instructions
            .iter()
            .flat_map(move |(instruction, cases)| {
                let instruction = *instruction;
                let cases: Box<dyn Iterator<Item = RecordedCase> + 'cases> = match cases {
                    Cases::Written(cases) => Box::new(cases.iter().cloned()),
                    Cases::Generated(cases) => {
                        Box::new(cases.generate_cases(generator, instruction))
                    }
                };
                cases.map(move |(before, after)| (instruction, before, after))
            })
    }

    /// Returns where the case recorded from exactly this processor and memory lies, looking first at the case after
    /// the one played last and at that one, then at the first case of each instruction, then everywhere.
    fn find_case(
        &self,
        processor: &Processor<()>,
        memory: &[MemoryDelta<'_, Arm64<User>>],
    ) -> Option<(usize, usize)> {
        let fingerprint = OnceCell::new();
        let is_recorded_from = |cases: &Cases, case: usize| match cases {
            Cases::Written(cases) => cases.get(case).is_some_and(|(before, _)| {
                before.processor == *processor && before.memory_mutations.as_slice() == memory
            }),
            Cases::Generated(cases) => cases.is_recorded_from(
                case,
                *fingerprint.get_or_init(|| to_fingerprint(processor, memory)),
            ),
        };
        let is_at = |&(instruction, case): &(usize, usize)| {
            self.instructions
                .get(instruction)
                .is_some_and(|(_, cases)| is_recorded_from(cases, case))
        };
        let likely = [Some(self.next), self.played];
        let first_cases = (0..self.instructions.len()).map(|instruction| Some((instruction, 0)));
        if let Some(found) = likely.into_iter().chain(first_cases).flatten().find(is_at) {
            return Some(found);
        }
        self.instructions
            .iter()
            .enumerate()
            .find_map(|(instruction, (_, cases))| {
                let case = match cases {
                    Cases::Written(_) => {
                        (0..cases.len()).find(|&case| is_recorded_from(cases, case))
                    }
                    Cases::Generated(generated) => generated
                        .find(*fingerprint.get_or_init(|| to_fingerprint(processor, memory))),
                }?;
                Some((instruction, case))
            })
    }
}

fn to_invalid_data(path: &Path, what: &dyn Debug) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        format!("{}: {what:?}", path.display()),
    )
}

/// Runs one state on the source, giving the state before and the state after.
fn run_case<Source: BehaviorSource<Architecture = Arm64<User>>>(
    source: &mut Source,
    before: ExecutionState<'static, Arm64<User>>,
) -> Result<RecordedCase, Source::Error> {
    let mut after = before.clone();
    source.execute_single_instruction_in_place(
        &mut after.processor,
        before.memory(),
        &mut after.memory_mutations,
    )?;
    Ok((before, after))
}

impl BehaviorSource for Recording {
    type Architecture = Arm64<User>;
    type Error = NotRecorded;

    /// Gives what the chip did from the recorded state equal to the processor and memory.
    fn execute_single_instruction_in_place(
        &mut self,
        processor: &mut Processor<()>,
        memory: MemorySnapshot<'_, '_, Arm64<User>>,
        mutation_hook: &mut impl MemorySink<Arm64<User>>,
    ) -> Result<(), NotRecorded> {
        let (instruction, case) = self
            .find_case(processor, memory.mutations)
            .ok_or(NotRecorded)?;
        let cases = &self.instructions[instruction].1;
        match cases {
            Cases::Written(written) => {
                let (before, after) = &written[case];
                processor.clone_from(&after.processor);
                for mutation in &after.memory_mutations[before.memory_mutations.len()..] {
                    mutation_hook.on_memory_mutated(mutation.clone());
                }
            }
            Cases::Generated(generated) => {
                let (after, made) = generated.play(case, processor).ok_or(NotRecorded)?;
                *processor = after;
                for mutation in made {
                    mutation_hook.on_memory_mutated(mutation);
                }
            }
        }
        self.played = Some((instruction, case));
        self.next = if case + 1 < cases.len() {
            (instruction, case + 1)
        } else {
            (instruction + 1, 0)
        };
        Ok(())
    }
}
