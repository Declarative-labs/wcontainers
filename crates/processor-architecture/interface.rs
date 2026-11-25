#![no_std]

extern crate alloc;

use alloc::borrow::Cow;
use alloc::vec::Vec;
#[cfg(verus_keep_ghost)]
use core::convert::Infallible;
use core::ops::Range;

#[cfg(verus_keep_ghost)]
use vstd::laws_eq::obeys_concrete_eq;
use vstd::prelude::*;
#[cfg(verus_keep_ghost)]
use vstd::std_specs::iter::IteratorSpec;

mod foreign_implementation {
    #[path = "vector/implementation.rs"]
    mod vector;
}

// VERUS: no proof apart from the code it proves, so the proofs, and the memory diff with its proof, are files of their own.
mod memory_diff;
#[cfg(verus_keep_ghost)]
pub mod memory_proof;
mod trusted;
#[cfg(test)]
mod trusted_tests;

#[cfg(verus_keep_ghost)]
use memory_proof::{lemma_appended_memory, lemma_execution_state_clone, lemma_state_equality};

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

    /// `==` on processors, on permissions and on instructions holds exactly for the same value.
    proof fn equality_is_identity()
        ensures
            obeys_concrete_eq::<Self::ProcessorState>(),
            obeys_concrete_eq::<Self::PagePermission>(),
            obeys_concrete_eq::<Self::Instruction>(),
    ;

    /// A processor's clone is the same value.
    proof fn clone_is_identity()
        ensures
            vstd::pervasive::obeys_clone_eq::<Self::ProcessorState>(),
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
    forall|memory: Memory<Architecture::PagePermission>|
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
    fn simulate_in_place(
        &self,
        processor: &mut Architecture::ProcessorState,
        memory: &[MemoryDelta<'_, Architecture>],
        mutation_hook: &mut impl MemorySink<Architecture>,
    ) -> (result: Result<(), Self::Error>)
        ensures
            result is Ok ==> (self.simulation(*old(processor), memory_of(memory@)) matches Ok((after, changes))
                && *final(processor) == after && receives(*old(mutation_hook), *final(mutation_hook), changes)),
    ;

    /// Simulates applying this instruction to the before state.
    fn simulate<'memory>(
        &self,
        mut before: ExecutionState<'memory, Architecture>,
    ) -> (after: Result<ExecutionState<'memory, Architecture>, Self::Error>)
        ensures
            after matches Ok(after) ==> (self.simulation(before.processor, memory_of(before.memory_mutations@))
                matches Ok((processor, changes)) && after.processor == processor
                && memory_of(after.memory_mutations@) == apply_all(memory_of(before.memory_mutations@), changes)),
    {
        let mut mutations = Vec::new();
        self.simulate_in_place(
            &mut before.processor,
            &before.memory_mutations,
            &mut mutations,
        )?;
        // VERUS: no fact about the memory of a joined list without its proof, so the proof is named.
        proof { lemma_appended_memory(before.memory_mutations@, mutations); }
        before.memory_mutations.append(&mut mutations);
        Ok(before)
    }
}

// VERUS: no contract on what a closure has received, so the memory hook is a trait.
pub trait MemorySink<Architecture: ProcessorArchitecture> {
    /// The memory after every change the sink has received, applied in order to the given memory.
    spec fn applied_to(&self, memory: Memory<Architecture::PagePermission>) -> Memory<Architecture::PagePermission>;

    fn on_memory_mutated(&mut self, mutation: MemoryDelta<'_, Architecture>)
        ensures
            forall|memory: Memory<Architecture::PagePermission>|
                #[trigger] final(self).applied_to(memory) == apply(old(self).applied_to(memory), mutation@),
    ;
}

/// All state that some instruction might mutate during execution.
pub struct ExecutionState<'memory, Architecture: ProcessorArchitecture> {
    pub processor: Architecture::ProcessorState,
    /// The changes that have been applied to the memory (as if it started from a fully zeroed out state), in chronological order.
    pub memory_mutations: Vec<MemoryDelta<'memory, Architecture>>,
}

// VERUS: no specification of a derived `Clone`, so `MemoryDelta`'s `Clone` is written out below.
#[derive(PartialEq, Eq)]
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

impl<Architecture: ProcessorArchitecture> View for MemoryDelta<'_, Architecture> {
    type V = Assignment<Architecture::PagePermission>;

    /// The change as the specification reads it.
    open spec fn view(&self) -> Assignment<Architecture::PagePermission> {
        match self {
            MemoryDelta::PermissionAssignment { addresses, permission } =>
                Assignment::PermissionAssignment { addresses: *addresses, permission: *permission },
            MemoryDelta::ValueAssignment { start, bytes } => Assignment::ValueAssignment { start: *start, bytes: bytes@ },
        }
    }
}

// VERUS: no specification of a derived `Clone`, so it is written out as derived.
impl<Architecture: ProcessorArchitecture> Clone for ExecutionState<'_, Architecture> {
    fn clone(&self) -> (cloned: Self)
        ensures
            cloned.processor == self.processor,
            memory_of(cloned.memory_mutations@) == memory_of(self.memory_mutations@),
    {
        let cloned = ExecutionState {
            processor: self.processor.clone(),
            memory_mutations: self.memory_mutations.clone(),
        };
        // VERUS: no fact that a clone of each part is the same value without its proof, so the proof is named.
        proof { lemma_execution_state_clone(*self, cloned); }
        cloned
    }
}

// VERUS: no specification of a derived `Clone`, so it is written out as derived.
impl<Architecture: ProcessorArchitecture> Clone for MemoryDelta<'_, Architecture> {
    fn clone(&self) -> (cloned: Self)
        ensures
            cloned@ == self@,
    {
        match self {
            Self::PermissionAssignment {
                addresses,
                permission,
            } => Self::PermissionAssignment {
                addresses: addresses.start..addresses.end,
                permission: *permission,
            },
            Self::ValueAssignment { start, bytes } => Self::ValueAssignment {
                start: *start,
                // VERUS: no `Cow::clone`, so its two cases are written out.
                bytes: match bytes {
                    Cow::Borrowed(borrowed) => Cow::Borrowed(*borrowed),
                    Cow::Owned(owned) => Cow::Owned(owned.clone()),
                },
            },
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
                    Cow::Borrowed(borrowed) => memory_diff::bytes_to_vec(borrowed),
                    Cow::Owned(owned) => owned,
                }),
            },
        }
    }

    /// Calls the hook with each mutation that turns the old memory snapshot into the new one, bytes before permissions.
    // VERUS: no `old(..)` of the hook where a parameter is named `old`, so its specification names `old` by its path.
    pub fn diff_snapshots(
        old: &[Self],
        new: &[Self],
        mutation_hook: &mut impl MemorySink<Architecture>,
    )
        ensures
            exists|changes: Seq<Assignment<Architecture::PagePermission>>| #![trigger apply_all(memory_of(old@), changes)]
                receives(*vstd::prelude::old(mutation_hook), *final(mutation_hook), changes)
                    && apply_all(memory_of(old@), changes) == memory_of(new@),
            memory_of(old@) == memory_of(new@) ==> *final(mutation_hook) == *vstd::prelude::old(mutation_hook),
    {
        // VERUS: no proof apart from the code it proves, so the body is memory_diff.rs, proven there.
        memory_diff::diff_snapshots(old, new, mutation_hook)
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
    fn eq(&self, other: &Self) -> bool {
        // VERUS: no facts of `==` and of an empty diff without their proof, so the proof is named.
        proof { lemma_state_equality::<Architecture>(); }
        if self.processor != other.processor {
            return false;
        }
        // VERUS: no closure that captures a mutable reference, so the mutations are collected and counted.
        let mut mutations = Vec::new();
        MemoryDelta::diff_snapshots(
            &self.memory_mutations,
            &other.memory_mutations,
            &mut mutations,
        );
        mutations.is_empty()
    }
}

impl<Architecture: ProcessorArchitecture> Eq for ExecutionState<'_, Architecture> {}

} // verus!
