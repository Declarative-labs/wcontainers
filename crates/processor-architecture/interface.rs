#![no_std]

extern crate alloc;

use alloc::borrow::Cow;
use alloc::vec::Vec;
#[cfg(verus_keep_ghost)]
use core::convert::Infallible;
use core::ops::Range;
use core::ptr;

use rangemap::RangeMap;
#[cfg(verus_keep_ghost)]
use vstd::laws_eq::obeys_concrete_eq;
use vstd::prelude::*;
#[cfg(verus_keep_ghost)]
use vstd::std_specs::iter::IteratorSpec;

/// The proofs about the memory model that this crate's methods use, which are not part of the specification.
#[cfg(verus_keep_ghost)]
pub mod memory_proof;

#[cfg(verus_keep_ghost)]
use memory_proof::*;

verus! {

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

    /// The instructions of the architecture, in the order they are listed.
    spec fn instructions() -> Seq<Self::Instruction>;

    /// The behaviors a chip is known to show for the instruction, in the order they are listed.
    spec fn known_behaviors(instruction: Self::Instruction) -> Seq<Self::PreciseInstruction>;

    /// `==` on processors and on permissions holds exactly for the same value.
    proof fn equality_is_identity()
        ensures
            obeys_concrete_eq::<Self::ProcessorState>(),
            obeys_concrete_eq::<Self::PagePermission>(),
    ;

    /// Returns the set of instructions of the architecture.
    fn list_instructions() -> (instructions: Self::Instructions)
        ensures
            instructions.obeys_prophetic_iter_laws(),
            instructions.remaining() == Self::instructions(),
            instructions.decrease() is Some,
            Self::instructions().no_duplicates(),
    ;

    /// Returns the set of behaviors a chip is known to show for the instruction.
    fn list_known_behaviors(instruction: Self::Instruction) -> (behaviors: Self::KnownBehaviors)
        ensures
            behaviors.obeys_prophetic_iter_laws(),
            behaviors.remaining() == Self::known_behaviors(instruction),
            behaviors.decrease() is Some,
            Self::known_behaviors(instruction).no_duplicates(),
    ;
}

/// One memory change as the specification reads it: a `MemoryDelta` with its bytes as a sequence.
pub ghost enum Assignment<Permission> {
    PermissionAssignment { addresses: Range<u64>, permission: Permission },
    ValueAssignment { start: u64, bytes: Seq<u8> },
}

/// Memory as the specification reads it: the byte and the permission at each address.
pub type Memory<Permission> = spec_fn(u64) -> (u8, Permission);

/// The memory no change has reached: every byte zero, every address with the default permission.
pub open spec fn zeroed<Architecture: ProcessorArchitecture>() -> Memory<Architecture::PagePermission> {
    |address: u64| (0u8, Architecture::DEFAULT_PERMISSION)
}

/// The memory after one change; a value assignment that runs past the last address goes on from address zero.
pub open spec fn apply<Permission>(memory: Memory<Permission>, change: Assignment<Permission>) -> Memory<Permission> {
    match change {
        Assignment::PermissionAssignment { addresses, permission } => |address: u64|
            if addresses.start <= address < addresses.end { (memory(address).0, permission) } else { memory(address) },
        Assignment::ValueAssignment { start, bytes } => |address: u64| {
            let offset = if start <= address { address - start } else { address + 0x1_0000_0000_0000_0000 - start };
            if offset < bytes.len() { (bytes[offset], memory(address).1) } else { memory(address) }
        },
    }
}

/// The memory after the changes, applied in order.
pub open spec fn apply_all<Permission>(memory: Memory<Permission>, changes: Seq<Assignment<Permission>>) -> Memory<Permission>
    decreases changes.len(),
{
    if changes.len() == 0 { memory } else { apply(apply_all(memory, changes.drop_last()), changes.last()) }
}

/// The memory a list of mutations leaves, applied in order to the zeroed memory.
pub open spec fn memory_of<'memory, Architecture: ProcessorArchitecture>(
    mutations: Seq<MemoryDelta<'memory, Architecture>>,
) -> Memory<Architecture::PagePermission> {
    apply_all(zeroed::<Architecture>(), assignments(mutations))
}

/// Each mutation as the specification reads it.
pub open spec fn assignments<'memory, Architecture: ProcessorArchitecture>(
    mutations: Seq<MemoryDelta<'memory, Architecture>>,
) -> Seq<Assignment<Architecture::PagePermission>> {
    mutations.map_values(|mutation: MemoryDelta<'memory, Architecture>| mutation@)
}

/// The processor and memory a behavior that cannot fail leaves from the state.
pub open spec fn simulated_after<'memory, Architecture, Behavior>(
    behavior: Behavior,
    before: ExecutionState<'memory, Architecture>,
) -> (Architecture::ProcessorState, Memory<Architecture::PagePermission>)
    where
        Architecture: ProcessorArchitecture,
        Behavior: Simulator<Architecture, Error = Infallible>,
{
    let memory = memory_of(before.memory_mutations@);
    match behavior.simulation(before.processor, memory) {
        Ok((processor, changes)) => (processor, apply_all(memory, changes)),
        // Never: the error is `Infallible`.
        Err(_) => (before.processor, memory),
    }
}

/// The processor and memory a state leaves once its processor becomes the outcome's and its memory receives the outcome's changes.
pub open spec fn state_after<'memory, Architecture: ProcessorArchitecture>(
    before: ExecutionState<'memory, Architecture>,
    outcome: (Architecture::ProcessorState, Seq<Assignment<Architecture::PagePermission>>),
) -> (Architecture::ProcessorState, Memory<Architecture::PagePermission>) {
    (outcome.0, apply_all(memory_of(before.memory_mutations@), outcome.1))
}

/// The sink went from `before` to `after` by receiving the changes, in order.
pub open spec fn receives<Architecture: ProcessorArchitecture, Sink: MemorySink<Architecture>>(
    before: Sink,
    after: Sink,
    changes: Seq<Assignment<Architecture::PagePermission>>,
) -> bool {
    Sink::obeys_sink_spec() ==> forall|memory: Memory<Architecture::PagePermission>|
        #[trigger] after.applied_to(memory) == apply_all(before.applied_to(memory), changes)
}

pub trait Simulator<Architecture: ProcessorArchitecture> {
    #[allow(
        reviewable::explicit_shared_error,
        reason = "simulate only runs simulate_in_place and passes its error on unchanged"
    )]
    type Error;

    /// The processor this instruction leaves and the changes it makes to memory, or its error, from the processor and memory before.
    spec fn simulation(
        &self,
        processor: Architecture::ProcessorState,
        memory: Memory<Architecture::PagePermission>,
    ) -> Result<(Architecture::ProcessorState, Seq<Assignment<Architecture::PagePermission>>), Self::Error>;

    /// Simulates applying this instruction to the given processor + memory.
    ///
    /// Processor changes are written in place, memory changes are written through the hook.
    /// On error the processor is left as it was.
    fn simulate_in_place(
        &self,
        processor: &mut Architecture::ProcessorState,
        memory: &[MemoryDelta<'_, Architecture>],
        mutation_hook: &mut impl MemorySink<Architecture>,
    ) -> (result: Result<(), Self::Error>)
        ensures
            match self.simulation(*old(processor), memory_of(memory@)) {
                Ok((after, changes)) => {
                    &&& result is Ok
                    &&& *final(processor) == after
                    &&& receives(*old(mutation_hook), *final(mutation_hook), changes)
                },
                Err(error) => result == Err::<(), Self::Error>(error) && *final(processor) == *old(processor),
            },
    ;

    /// Simulates applying this instruction to the before state.
    fn simulate<'memory>(
        &self,
        before: ExecutionState<'memory, Architecture>,
    ) -> (after: Result<ExecutionState<'memory, Architecture>, Self::Error>)
        ensures
            match self.simulation(before.processor, memory_of(before.memory_mutations@)) {
                Ok((processor, changes)) => after matches Ok(after) && after.processor == processor
                    && memory_of(after.memory_mutations@) == apply_all(memory_of(before.memory_mutations@), changes),
                Err(error) => after == Err::<ExecutionState<'memory, Architecture>, Self::Error>(error),
            },
    {
        // VERUS: no `mut` on an argument a specification reads, so the state is moved into a local.
        let mut before = before;
        let ghost memory = before.memory_mutations@;
        let ghost processor = before.processor;
        let mut mutations: Vec<MemoryDelta<'memory, Architecture>> = Vec::new();
        let ghost empty = mutations;
        let result = self.simulate_in_place(
            &mut before.processor,
            before.memory_mutations.as_slice(),
            &mut mutations,
        );
        // VERUS: no `?` whose error a specification compares, so the error is matched.
        match result {
            Ok(()) => {},
            Err(error) => return Err(error),
        }
        let ghost filled = mutations;
        before.memory_mutations.append(&mut mutations);
        proof {
            if let Ok((_, changes)) = self.simulation(processor, memory_of(memory)) {
                assert(filled.applied_to(memory_of(memory)) == apply_all(empty.applied_to(memory_of(memory)), changes));
                lemma_no_assignments(empty@, memory_of(memory));
                lemma_appended_memory(memory, filled@);
            }
        }
        Ok(before)
    }
}

// VERUS: no contract on what a closure has received, so the memory hook is a trait.
pub trait MemorySink<Architecture: ProcessorArchitecture> {
    /// Whether the sink holds what it received where a specification can read it, which a closure does not.
    spec fn obeys_sink_spec() -> bool;

    /// The memory after every change the sink has received, applied in order to the given memory.
    spec fn applied_to(&self, memory: Memory<Architecture::PagePermission>) -> Memory<Architecture::PagePermission>;

    fn on_memory_mutated(&mut self, mutation: MemoryDelta<'_, Architecture>)
        ensures
            Self::obeys_sink_spec() ==> forall|memory: Memory<Architecture::PagePermission>|
                #[trigger] final(self).applied_to(memory) == apply(old(self).applied_to(memory), mutation@),
    ;
}

// VERUS: no closure that captures a mutable reference, so a list of changes is a hook itself.
impl<'memory, Architecture: ProcessorArchitecture> MemorySink<Architecture>
    for Vec<MemoryDelta<'memory, Architecture>>
{
    open spec fn obeys_sink_spec() -> bool {
        true
    }

    open spec fn applied_to(&self, memory: Memory<Architecture::PagePermission>) -> Memory<Architecture::PagePermission> {
        apply_all(memory, assignments(self@))
    }

    fn on_memory_mutated(&mut self, mutation: MemoryDelta<'_, Architecture>) {
        let ghost before = self@;
        let ghost change = mutation@;
        self.push(mutation.into_owned());
        proof {
            assert(assignments(self@).drop_last() =~= assignments(before));
            assert(assignments(self@).last() == change);
        }
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

impl<'memory, Architecture: ProcessorArchitecture> View for MemoryDelta<'memory, Architecture> {
    type V = Assignment<Architecture::PagePermission>;

    open spec fn view(&self) -> Assignment<Architecture::PagePermission> {
        match self {
            MemoryDelta::PermissionAssignment { addresses, permission } =>
                Assignment::PermissionAssignment { addresses: *addresses, permission: *permission },
            MemoryDelta::ValueAssignment { start, bytes } => Assignment::ValueAssignment { start: *start, bytes: bytes@ },
        }
    }
}

impl<Architecture: ProcessorArchitecture> MemoryDelta<'_, Architecture> {
    pub fn into_owned(self) -> (owned: MemoryDelta<'static, Architecture>)
        ensures
            owned@ == self@,
    {
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
                // VERUS: no `Cow::into_owned`, so its two cases are written out.
                bytes: Cow::Owned(match bytes {
                    Cow::Borrowed(borrowed) => vstd::slice::slice_to_vec(borrowed),
                    Cow::Owned(owned) => owned,
                }),
            },
        }
    }
}

#[cfg(verus_keep_ghost)]
impl<Architecture: ProcessorArchitecture> vstd::std_specs::cmp::PartialEqSpecImpl for ExecutionState<'_, Architecture> {
    open spec fn obeys_eq_spec() -> bool {
        true
    }

    /// The same processor, and the same byte and permission at every address.
    open spec fn eq_spec(&self, other: &Self) -> bool {
        &&& self.processor == other.processor
        &&& memory_of(self.memory_mutations@) == memory_of(other.memory_mutations@)
    }
}

/// Two states are equal when they hold the same processor and the same memory, regardless of the order of changes.
impl<Architecture: ProcessorArchitecture> PartialEq for ExecutionState<'_, Architecture> {
    fn eq(&self, other: &Self) -> (equal: bool) {
        proof {
            Architecture::equality_is_identity();
            reveal(obeys_concrete_eq);
        }
        if self.processor != other.processor {
            return false;
        }
        // VERUS: no closure that captures a mutable reference, so the mutations are collected and counted.
        let mut mutations: Vec<MemoryDelta<'_, Architecture>> = Vec::new();
        let ghost empty = mutations;
        diff_memory(
            self.memory_mutations.as_slice(),
            other.memory_mutations.as_slice(),
            &mut mutations,
        );
        proof {
            lemma_diff_into_empty(memory_of(self.memory_mutations@), memory_of(other.memory_mutations@), empty, mutations);
        }
        mutations.is_empty()
    }
}

impl<Architecture: ProcessorArchitecture> Eq for ExecutionState<'_, Architecture> {}

/// Calls the hook with each mutation that turns the memory of the first list into the memory of the second, bytes before permissions.
#[cfg_attr(verus_keep_ghost, verus::trusted)]
#[verifier::external_body]
pub fn diff_memory<Architecture: ProcessorArchitecture>(
    first: &[MemoryDelta<'_, Architecture>],
    second: &[MemoryDelta<'_, Architecture>],
    mutation_hook: &mut impl MemorySink<Architecture>,
)
    ensures
        exists|changes: Seq<Assignment<Architecture::PagePermission>>| #![trigger apply_all(memory_of(first@), changes)]
            receives(*old(mutation_hook), *final(mutation_hook), changes)
                && apply_all(memory_of(first@), changes) == memory_of(second@),
        memory_of(first@) == memory_of(second@) ==> *final(mutation_hook) == *old(mutation_hook),
{
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

// A closure promises nothing to a specification, so calling it is trusted.
impl<Architecture: ProcessorArchitecture, Hook: FnMut(MemoryDelta<'_, Architecture>)>
    MemorySink<Architecture> for Hook
{
    open spec fn obeys_sink_spec() -> bool {
        false
    }

    open spec fn applied_to(&self, memory: Memory<Architecture::PagePermission>) -> Memory<Architecture::PagePermission> {
        memory
    }

    #[cfg_attr(verus_keep_ghost, verus::trusted)]
    #[verifier::external_body]
    fn on_memory_mutated(&mut self, mutation: MemoryDelta<'_, Architecture>) {
        self(mutation)
    }
}

} // verus!
