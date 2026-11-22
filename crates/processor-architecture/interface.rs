#![no_std]

extern crate alloc;

use alloc::vec::Vec;
use core::convert::Infallible;
use core::ops::Range;

/// One processor architecture, as a program running on it sees it.
pub trait ProcessorArchitecture: Sized {
    /// All stateful components of the processor.
    type Processor: Clone + Eq;
    /// What a program may do with a page of memory.
    type PagePermission: Copy + Eq;
    /// An instruction as the architecture's manual documents it.
    type Instruction: Copy + Eq;
    /// An instruction with a specific pinned behaviour that should be expected of it.
    type PreciseInstruction: Simulator<Self, Error = Infallible> + Copy + Eq;

    /// The addresses a program can use.
    const ADDRESSABLE: Range<u64>;

    /// The order means nothing.
    fn list_instructions() -> impl Iterator<Item = Self::Instruction>;
}

pub trait Simulator<Architecture: ProcessorArchitecture> {
    type Error;

    /// Simulates applying this instruction to the before state.
    fn simulate(
        &self,
        before: ExecutionState<Architecture>,
    ) -> Result<ExecutionState<Architecture>, Self::Error>;
}

/// All state that some instruction might mutate during execution.
#[derive(Clone, PartialEq, Eq)]
pub struct ExecutionState<Architecture: ProcessorArchitecture> {
    pub processor: Architecture::Processor,
    /// The changes that have been applied to the memory (as if it started from a fully zeroed out state), in chronological order.
    pub memory_mutations: Vec<MemoryDelta<Architecture>>,
}

#[derive(Clone, PartialEq, Eq)]
pub enum MemoryDelta<Architecture: ProcessorArchitecture> {
    PermissionAssignment {
        addresses: Range<u64>,
        permission: Architecture::PagePermission,
    },
    ValueAssignment {
        start: u64,
        bytes: Vec<u8>,
    },
}
