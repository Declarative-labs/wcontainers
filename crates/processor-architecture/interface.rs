#![no_std]

extern crate alloc;

use alloc::borrow::Cow;
use alloc::vec::Vec;
use core::ops::Range;

use litemap::LiteMap;
use rangemap::RangeMap;

/// One processor architecture, as a program running on it sees it.
pub trait ProcessorArchitecture: Sized {
    /// All stateful components of the processor.
    type ProcessorState: Clone + Eq;
    /// What a program may do with a page of memory.
    type PagePermission: Copy + Eq;
    /// An instruction at its most granular division.
    ///
    /// On A64 this is an [instruction variant](https://developer.arm.com/documentation/ddi0487/mc/-Part-C-The-AArch64-Instruction-Set/-Chapter-C1-The-A64-Instruction-Set/-C1-1-About-the-A64-instruction-set#:~:text=each%20syntax%20is%20an%20instruction%20variant), on x86 an [instruction form](https://intelxed.github.io/ref-manual/group__IFORM.html).
    type Instruction: Copy + Eq;
    /// An instruction with a specific pinned behavior that should be expected of it.
    // VERUS: no trait that refers back to itself, so code that simulates states `Simulator` itself.
    type PreciseInstruction: Copy + Eq;
    // VERUS: no `impl Trait` in a trait, so each returned iterator is a named type.
    type Instructions: Iterator<Item = Self::Instruction>;
    type KnownBehaviors: Iterator<Item = Self::PreciseInstruction>;

    /// The addresses a program can use.
    const ADDRESSABLE: Range<u64>;
    const DEFAULT_PERMISSION: Self::PagePermission;

    /// Returns the set of instructions of the architecture.
    fn list_instructions() -> Self::Instructions;

    /// Returns the set of behaviors a chip is known to show for the instruction.
    fn list_known_behaviors(instruction: Self::Instruction) -> Self::KnownBehaviors;
}

pub trait Simulator<Architecture: ProcessorArchitecture> {
    #[allow(
        reviewable::explicit_shared_error,
        reason = "simulate only runs simulate_in_place and passes its error on unchanged"
    )]
    type Error;

    /// Simulates applying this instruction to the given processor + memory.
    ///
    /// Processor changes are written in place, memory changes are written through the hook.
    fn simulate_in_place(
        &self,
        processor: &mut Architecture::ProcessorState,
        memory: &[MemoryDelta<'_, Architecture>],
        mutation_hook: &mut impl MemorySink<Architecture>,
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
            &mut mutations,
        )?;
        before.memory_mutations.append(&mut mutations);
        Ok(before)
    }
}

// VERUS: no contract on what a closure has received, so the memory hook is a trait.
pub trait MemorySink<Architecture: ProcessorArchitecture> {
    fn on_memory_mutated(&mut self, mutation: MemoryDelta<'_, Architecture>);
}

impl<Architecture: ProcessorArchitecture, Hook: FnMut(MemoryDelta<'_, Architecture>)>
    MemorySink<Architecture> for Hook
{
    fn on_memory_mutated(&mut self, mutation: MemoryDelta<'_, Architecture>) {
        self(mutation)
    }
}

// VERUS: no closure that captures a mutable reference, so a list of changes is a hook itself.
impl<Architecture: ProcessorArchitecture> MemorySink<Architecture>
    for Vec<MemoryDelta<'_, Architecture>>
{
    fn on_memory_mutated(&mut self, mutation: MemoryDelta<'_, Architecture>) {
        self.push(mutation.into_owned())
    }
}

/// All state that some instruction might mutate during execution.
#[derive(Clone)]
pub struct ExecutionState<'memory, Architecture: ProcessorArchitecture> {
    pub processor: Architecture::ProcessorState,
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

/// Two states are equal when they hold the same processor and the same memory, regardless of the order of changes.
impl<Architecture: ProcessorArchitecture> PartialEq for ExecutionState<'_, Architecture> {
    fn eq(&self, other: &Self) -> bool {
        /// Returns whether the two changes assign the same thing to the same addresses.
        fn is_same_change<Architecture: ProcessorArchitecture>(
            first: &MemoryDelta<'_, Architecture>,
            second: &MemoryDelta<'_, Architecture>,
        ) -> bool {
            match (first, second) {
                (
                    MemoryDelta::PermissionAssignment {
                        addresses: first_addresses,
                        permission: first_permission,
                    },
                    MemoryDelta::PermissionAssignment {
                        addresses: second_addresses,
                        permission: second_permission,
                    },
                ) => first_addresses == second_addresses && first_permission == second_permission,
                (
                    MemoryDelta::ValueAssignment {
                        start: first_start,
                        bytes: first_bytes,
                    },
                    MemoryDelta::ValueAssignment {
                        start: second_start,
                        bytes: second_bytes,
                    },
                ) => first_start == second_start && first_bytes == second_bytes,
                _ => false,
            }
        }

        /// Returns the byte the last change that covers the address assigned, which is zero where none did.
        fn find_byte<Architecture: ProcessorArchitecture>(
            mutations: &[MemoryDelta<'_, Architecture>],
            address: u64,
        ) -> u8 {
            mutations
                .iter()
                .rev()
                .find_map(|mutation| match mutation {
                    MemoryDelta::ValueAssignment { start, bytes } => {
                        let offset = address.wrapping_sub(*start);
                        (offset < bytes.len() as u64).then(|| bytes[offset as usize])
                    }
                    MemoryDelta::PermissionAssignment { .. } => None,
                })
                .unwrap_or(u8::MIN)
        }

        /// Returns the byte the state holds at each address that the given changes write, keyed by memory address.
        fn resolve_written_memory<Architecture: ProcessorArchitecture>(
            mutations: &[MemoryDelta<'_, Architecture>],
            written: [&[MemoryDelta<'_, Architecture>]; 2],
        ) -> LiteMap<u64, u8> {
            let mut rendered = LiteMap::new_vec();
            for mutation in written.into_iter().flatten() {
                if let MemoryDelta::ValueAssignment { start, bytes } = mutation {
                    for offset in 0..bytes.len() as u64 {
                        let address = start.wrapping_add(offset);
                        rendered.insert(address, find_byte(mutations, address));
                    }
                }
            }
            rendered
        }

        /// Returns the permission of every range of addresses once the changes are applied, keyed by address range.
        fn resolve_permissions<Architecture: ProcessorArchitecture>(
            mutations: &[MemoryDelta<'_, Architecture>],
        ) -> RangeMap<u64, Architecture::PagePermission> {
            let mut rendered = RangeMap::new();
            for mutation in mutations {
                if let MemoryDelta::PermissionAssignment {
                    addresses,
                    permission,
                } = mutation
                    && !addresses.is_empty()
                {
                    rendered.insert(addresses.clone(), *permission);
                }
            }
            rendered
        }

        // The changes both states start with leave the same memory in both, so only what follows them can differ.
        let shared = self
            .memory_mutations
            .iter()
            .zip(&other.memory_mutations)
            .take_while(|(first, second)| is_same_change(first, second))
            .count();
        let written = [
            &self.memory_mutations[shared..],
            &other.memory_mutations[shared..],
        ];
        self.processor == other.processor
            && resolve_written_memory(&self.memory_mutations, written)
                == resolve_written_memory(&other.memory_mutations, written)
            && resolve_permissions(&self.memory_mutations)
                == resolve_permissions(&other.memory_mutations)
    }
}

impl<Architecture: ProcessorArchitecture> Eq for ExecutionState<'_, Architecture> {}
