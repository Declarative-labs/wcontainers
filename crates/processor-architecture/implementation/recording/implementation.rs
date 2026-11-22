//! What one chip did with one instruction on each state it was given.

use wcontainers_distinguishing_state_generator::DistinguishingStateGenerator;
use wcontainers_executor::Executor;
use wcontainers_processor_architecture::{
    ExecutionState, MemoryDelta, ProcessorArchitecture, Simulator,
};

mod arm64;
#[cfg(test)]
mod tests;

pub use arm64::{MalformedRecording, from_text, read_recordings, to_text, write_recordings};

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
        processor: &mut Architecture::Processor,
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

/// Whether two lists of changes, each applied after `memory`, leave the same bytes and permissions.
///
/// The lists themselves may differ: a chip reports a byte rewritten with the value it held as no change.
pub fn leave_same_memory<Architecture: ProcessorArchitecture>(
    memory: &[MemoryDelta<'_, Architecture>],
    first: &[MemoryDelta<'_, Architecture>],
    second: &[MemoryDelta<'_, Architecture>],
) -> bool {
    let all = [memory, first, second];
    first.iter().chain(second).all(|mutation| match mutation {
        MemoryDelta::ValueAssignment { start, bytes } => (0..bytes.len() as u64).all(|offset| {
            let address = start.wrapping_add(offset);
            read_byte([memory, first], address) == read_byte([memory, second], address)
        }),
        // A permission is the same across each run of addresses that no assignment starts or ends inside.
        MemoryDelta::PermissionAssignment { addresses, .. } => {
            let boundaries = all
                .into_iter()
                .flatten()
                .filter_map(|mutation| match mutation {
                    MemoryDelta::PermissionAssignment { addresses, .. } => {
                        Some([addresses.start, addresses.end])
                    }
                    MemoryDelta::ValueAssignment { .. } => None,
                });
            let inside = boundaries
                .flatten()
                .filter(|boundary| addresses.contains(boundary));
            [addresses.start].into_iter().chain(inside).all(|address| {
                read_permission([memory, first], address)
                    == read_permission([memory, second], address)
            })
        }
    })
}

/// Reads a byte of the memory the second list of changes leaves after the first.
fn read_byte<Architecture: ProcessorArchitecture>(
    lists: [&[MemoryDelta<'_, Architecture>]; 2],
    address: u64,
) -> u8 {
    lists
        .into_iter()
        .rev()
        .flat_map(|list| list.iter().rev())
        .find_map(|mutation| match mutation {
            MemoryDelta::ValueAssignment { start, bytes } => {
                usize::try_from(address.wrapping_sub(*start))
                    .ok()
                    .and_then(|offset| bytes.get(offset))
                    .copied()
            }
            MemoryDelta::PermissionAssignment { .. } => None,
        })
        .unwrap_or_default()
}

/// Reads the permission of an address, or none when no change gave it one.
fn read_permission<Architecture: ProcessorArchitecture>(
    lists: [&[MemoryDelta<'_, Architecture>]; 2],
    address: u64,
) -> Option<Architecture::PagePermission> {
    lists
        .into_iter()
        .rev()
        .flat_map(|list| list.iter().rev())
        .find_map(|mutation| match mutation {
            MemoryDelta::PermissionAssignment {
                addresses,
                permission,
            } => addresses.contains(&address).then_some(*permission),
            MemoryDelta::ValueAssignment { .. } => None,
        })
}
