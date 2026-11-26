//! A state generator that counts every state it gives, so the command can say how many were measured.

use std::sync::atomic::{AtomicU64, Ordering};

use wcontainers_distinguishing_state_generator::DistinguishingStateGenerator;
use wcontainers_processor_architecture::ProcessorArchitecture;

/// The states of a generator, each counted as it is given.
pub struct CountedGenerator<'count, Generator> {
    pub generator: Generator,
    pub states: &'count AtomicU64,
}

impl<Architecture, Generator> DistinguishingStateGenerator<Architecture>
    for CountedGenerator<'_, Generator>
where
    Architecture: ProcessorArchitecture,
    Generator: DistinguishingStateGenerator<Architecture>,
{
    type States<'generator>
        = CountedStates<'generator, Generator::States<'generator>>
    where
        Self: 'generator;

    fn generate_states(&self, instruction: Architecture::Instruction) -> Self::States<'_> {
        CountedStates {
            states: self.generator.generate_states(instruction),
            count: self.states,
        }
    }
}

/// The states of one instruction, each counted as it is given.
pub struct CountedStates<'count, States> {
    states: States,
    count: &'count AtomicU64,
}

impl<States: Iterator> Iterator for CountedStates<'_, States> {
    type Item = States::Item;

    fn next(&mut self) -> Option<States::Item> {
        let state = self.states.next()?;
        self.count.fetch_add(1, Ordering::Relaxed);
        Some(state)
    }
}
