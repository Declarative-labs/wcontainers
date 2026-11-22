#![no_std]

extern crate alloc;

use alloc::borrow::Cow;
use alloc::vec::Vec;
use core::convert::Infallible;
use core::ops::Range;

/// One processor architecture, as a program running on it sees it.
pub trait ProcessorArchitecture: Sized {
    /// All stateful components of the processor.
    type Processor: Clone + Eq;
    /// What a program may do with a page of memory.
    type PagePermission: Copy + Eq;
    /// An instruction at its most granular division.
    ///
    /// On A64 this is an [instruction variant](https://developer.arm.com/documentation/ddi0487/mc/-Part-C-The-AArch64-Instruction-Set/-Chapter-C1-The-A64-Instruction-Set/-C1-1-About-the-A64-instruction-set#:~:text=each%20syntax%20is%20an%20instruction%20variant), on x86 an [instruction form](https://intelxed.github.io/ref-manual/group__IFORM.html).
    type Instruction: Copy + Eq;
    /// An instruction with a specific pinned behavior that should be expected of it.
    type PreciseInstruction: Simulator<Self, Error = Infallible> + Copy + Eq;

    /// The addresses a program can use.
    const ADDRESSABLE: Range<u64>;

    /// The order means nothing.
    fn list_instructions() -> impl Iterator<Item = Self::Instruction>;
}

pub trait Simulator<Architecture: ProcessorArchitecture> {
    type Error;

    /// Simulates applying this instruction to the given processor + memory.
    ///
    /// Processor changes are written in place, memory changes are written through the hook.
    /// On error the processor is left as it was.
    fn simulate_in_place(
        &self,
        processor: &mut Architecture::Processor,
        memory: &[MemoryDelta<'_, Architecture>],
        on_memory_mutated: impl FnMut(MemoryDelta<'_, Architecture>),
    ) -> Result<(), Self::Error>;

    /// Simulates applying this instruction to the before state.
    fn simulate<'memory>(
        &self,
        mut before: ExecutionState<'memory, Architecture>,
    ) -> Result<ExecutionState<'memory, Architecture>, Self::Error> {
        let mut mutations = Vec::new();
        self.simulate_in_place(
            &mut before.processor,
            &before.memory_mutations,
            |mutation| mutations.push(mutation.into_owned()),
        )?;
        before.memory_mutations.append(&mut mutations);
        Ok(before)
    }
}

/// All state that some instruction might mutate during execution.
#[derive(Clone, PartialEq, Eq)]
pub struct ExecutionState<'memory, Architecture: ProcessorArchitecture> {
    pub processor: Architecture::Processor,
    /// The changes that have been applied to the memory (as if it started from a fully zeroed out state), in chronological order.
    pub memory_mutations: Vec<MemoryDelta<'memory, Architecture>>,
}

#[derive(Clone, PartialEq, Eq)]
pub enum MemoryDelta<'memory, Architecture: ProcessorArchitecture> {
    PermissionAssignment {
        addresses: Range<u64>,
        permission: Architecture::PagePermission,
    },
    ValueAssignment {
        start: u64,
        bytes: Cow<'memory, [u8]>,
    },
}

impl<Architecture: ProcessorArchitecture> MemoryDelta<'_, Architecture> {
    pub fn into_owned(self) -> MemoryDelta<'static, Architecture> {
        match self {
            Self::PermissionAssignment {
                addresses,
                permission,
            } => MemoryDelta::PermissionAssignment {
                addresses,
                permission,
            },
            Self::ValueAssignment { start, bytes } => MemoryDelta::ValueAssignment {
                start,
                bytes: Cow::Owned(bytes.into_owned()),
            },
        }
    }
}
