#![no_std]

extern crate alloc;

use alloc::borrow::Cow;
use alloc::vec::Vec;
use core::ops::Range;
use core::ptr;

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
    /// On error the processor is left as it was.
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
        if self.processor != other.processor {
            return false;
        }
        // VERUS: no closure that captures a mutable reference, so the mutations are collected and counted.
        let mut mutations = Vec::new();
        diff_memory(
            &self.memory_mutations,
            &other.memory_mutations,
            &mut mutations,
        );
        mutations.is_empty()
    }
}

impl<Architecture: ProcessorArchitecture> Eq for ExecutionState<'_, Architecture> {}

/// Calls the hook with each mutation that turns the memory of the first list into the memory of the second, bytes before permissions.
pub fn diff_memory<Architecture: ProcessorArchitecture>(
    first: &[MemoryDelta<'_, Architecture>],
    second: &[MemoryDelta<'_, Architecture>],
    mutation_hook: &mut impl MemorySink<Architecture>,
) {
    /// Returns whether the two mutations assign the same thing to the same addresses, the same slice of bytes first.
    fn is_same_mutation<Architecture: ProcessorArchitecture>(
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
            ) => {
                first_start == second_start
                    && (ptr::eq(&**first_bytes, &**second_bytes) || first_bytes == second_bytes)
            }
            _ => false,
        }
    }

    /// Records the addresses an unpaired mutation reaches: the bytes it writes, or that it assigns a permission.
    // VERUS: no closure that captures a mutable reference, so what is recorded is passed in.
    fn add_unpaired<Architecture: ProcessorArchitecture>(
        mutation: &MemoryDelta<'_, Architecture>,
        written: &mut Vec<Range<u128>>,
        has_unpaired_permission: &mut bool,
    ) {
        match mutation {
            MemoryDelta::ValueAssignment { start, bytes } => {
                let end = u128::from(*start) + bytes.len() as u128;
                // A write that runs past the last address goes on from zero, in a second range.
                let ranges_around_wrap = [
                    u128::from(*start)..end.min(ADDRESSES_END),
                    0..end.saturating_sub(ADDRESSES_END),
                ];
                written.extend(
                    ranges_around_wrap
                        .into_iter()
                        .filter(|addresses| !addresses.is_empty()),
                );
            }
            MemoryDelta::PermissionAssignment { addresses, .. } => {
                *has_unpaired_permission |= !addresses.is_empty();
            }
        }
    }

    /// Writes into `rendered` what the mutations leave at its addresses from `start` on, which is zero where none wrote.
    fn resolve_bytes<Architecture: ProcessorArchitecture>(
        mutations: &[MemoryDelta<'_, Architecture>],
        start: u64,
        length: usize,
        rendered: &mut Vec<u8>,
    ) {
        rendered.clear();
        rendered.resize(length, u8::MIN);
        for mutation in mutations {
            if let MemoryDelta::ValueAssignment {
                start: mutation_start,
                bytes,
            } = mutation
            {
                // Addresses wrap after the last one, so the offsets are taken modulo 2^64.
                let rendered_start_in_bytes = start.wrapping_sub(*mutation_start);
                let bytes_start_in_rendered = mutation_start.wrapping_sub(start);
                if rendered_start_in_bytes < bytes.len() as u64 {
                    let copied = (bytes.len() - rendered_start_in_bytes as usize).min(length);
                    rendered[..copied]
                        .copy_from_slice(&bytes[rendered_start_in_bytes as usize..][..copied]);
                } else if bytes_start_in_rendered < length as u64 {
                    let copied = bytes.len().min(length - bytes_start_in_rendered as usize);
                    rendered[bytes_start_in_rendered as usize..][..copied]
                        .copy_from_slice(&bytes[..copied]);
                }
            }
        }
    }

    /// Returns the permission of every range of addresses once the mutations are applied, keyed by address range.
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

    // Mutations both lists make in the same order leave the same memory wherever no other mutation reaches, so only
    // what the unpaired mutations reach can differ.
    const ADDRESSES_END: u128 = 1 << u64::BITS;
    let (mut written, mut has_unpaired_permission) = (Vec::new(), false);
    let mut rest = second;
    for mutation in first {
        match rest
            .iter()
            .position(|other| is_same_mutation(mutation, other))
        {
            Some(index) => {
                for skipped in &rest[..index] {
                    add_unpaired(skipped, &mut written, &mut has_unpaired_permission);
                }
                rest = &rest[index + 1..];
            }
            None => add_unpaired(mutation, &mut written, &mut has_unpaired_permission),
        }
    }
    for unpaired in rest {
        add_unpaired(unpaired, &mut written, &mut has_unpaired_permission);
    }
    written.sort_unstable_by_key(|addresses| addresses.start);
    // Overlapping and touching ranges become one, so that each address is compared once.
    written.dedup_by(|addresses, merged| {
        let touching = addresses.start <= merged.end;
        if touching {
            merged.end = merged.end.max(addresses.end);
        }
        touching
    });
    let (mut first_bytes, mut second_bytes) = (Vec::new(), Vec::new());
    for addresses in written {
        let start = addresses.start as u64;
        let length = (addresses.end - addresses.start) as usize;
        resolve_bytes(first, start, length, &mut first_bytes);
        resolve_bytes(second, start, length, &mut second_bytes);
        let is_same_at = |offset: &usize| first_bytes[*offset] == second_bytes[*offset];
        // Each run of bytes the two lists leave different is one mutation.
        let mut run_end = usize::MIN;
        while let Some(run_start) = (run_end..length).find(|offset| !is_same_at(offset)) {
            run_end = (run_start..length).find(is_same_at).unwrap_or(length);
            mutation_hook.on_memory_mutated(MemoryDelta::ValueAssignment {
                start: start + run_start as u64,
                bytes: Cow::Borrowed(&second_bytes[run_start..run_end]),
            });
        }
    }
    if !has_unpaired_permission {
        return;
    }
    let first_permissions = resolve_permissions(first);
    let second_permissions = resolve_permissions(second);
    let mut bounds: Vec<u64> = first_permissions
        .iter()
        .chain(second_permissions.iter())
        .flat_map(|(addresses, _)| [addresses.start, addresses.end])
        .collect();
    bounds.sort_unstable();
    bounds.dedup();
    let read_permission = |permissions: &RangeMap<u64, Architecture::PagePermission>, address| {
        *permissions
            .get(&address)
            .unwrap_or(&Architecture::DEFAULT_PERMISSION)
    };
    // Between two neighboring bounds each list leaves one permission.
    let mut bounds = bounds.into_iter();
    let Some(mut start) = bounds.next() else {
        return;
    };
    for end in bounds {
        let addresses = start..end;
        start = end;
        let permission = read_permission(&second_permissions, addresses.start);
        if read_permission(&first_permissions, addresses.start) != permission {
            mutation_hook.on_memory_mutated(MemoryDelta::PermissionAssignment {
                addresses,
                permission,
            });
        }
    }
}
