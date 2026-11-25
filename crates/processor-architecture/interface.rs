#![no_std]

extern crate alloc;

mod foreign_implementation {
    #[path = "vector/implementation.rs"]
    mod vector;
}

use alloc::borrow::Cow;
use alloc::vec::Vec;
#[cfg(verus_keep_ghost)]
use core::convert::Infallible;
use core::ops::Range;

#[cfg(verus_keep_ghost)]
use vstd::laws_eq::obeys_concrete_eq;
#[cfg(verus_keep_ghost)]
use vstd::std_specs::cmp::PartialEqSpec;
use vstd::prelude::*;
#[cfg(verus_keep_ghost)]
use vstd::std_specs::iter::IteratorSpec;

// `MemorySnapshot::diff`, with its proof.
mod memory_diff;
mod trusted;
#[cfg(test)]
mod trusted_tests;

verus! {

/// One processor architecture, as a program running on it sees it.
pub trait ProcessorArchitecture: Sized + Clone {
    /// All stateful components of the processor.
    type ProcessorState: Clone + Eq;
    /// What a behavior expects the processor to hold after an instruction.
    type ProcessorStateExpectation: Clone + Eq;
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

    /// `==` on permissions and on instructions holds exactly for the same value.
    proof fn equality_is_identity()
        ensures
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
    ;

    /// Returns the set of behaviors a chip is known to show for the instruction.
    fn list_known_behaviors(instruction: Self::Instruction) -> (behaviors: Self::KnownBehaviors)
        ensures
            behaviors.obeys_prophetic_iter_laws(),
            behaviors.remaining() == Self::known_behaviors(instruction),
            behaviors.decrease() is Some,
    ;

    /// The observed processor has every value the expectation holds.
    spec fn meets(observed: Self::ProcessorState, expectation: Self::ProcessorStateExpectation) -> bool;

    /// Returns whether the observed processor has every value the expectation holds.
    fn meets_expectations(
        observed: &Self::ProcessorState,
        expectation: &Self::ProcessorStateExpectation,
    ) -> (meets: bool)
        ensures
            meets == Self::meets(*observed, *expectation),
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

/// Applying two lists of changes one after the other is applying the joined list.
pub proof fn lemma_apply_all_append<Permission>(memory: Memory<Permission>, first: Seq<Assignment<Permission>>, second: Seq<Assignment<Permission>>)
    ensures
        apply_all(memory, first + second) == apply_all(apply_all(memory, first), second),
    decreases second.len(),
{
    if second.len() == 0 {
        assert(first + second =~= first);
    } else {
        lemma_apply_all_append(memory, first, second.drop_last());
        assert((first + second).drop_last() =~= first + second.drop_last());
    }
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

/// The processor a behavior that cannot fail expects and the memory it leaves from the state.
pub open spec fn simulated_after<'memory, Architecture, Behavior>(
    behavior: Behavior,
    before: ExecutionState<'memory, Architecture>,
) -> (Architecture::ProcessorStateExpectation, Memory<Architecture::PagePermission>)
    where
        Architecture: ProcessorArchitecture,
        Behavior: Simulator<Architecture, Error = Infallible>,
{
    let memory = memory_of(before.memory_mutations@);
    match behavior.simulation(before.processor, memory) {
        Ok((processor, changes)) => (processor, apply_all(memory, changes)),
        // Never: the error is `Infallible`.
        Err(_) => arbitrary(),
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
        reason = "simulate_state only runs simulate and passes its error on unchanged"
    )]
    type Error;

    /// The processor this instruction expects and the changes it makes to memory, or its error, from the processor and memory before.
    spec fn simulation(
        &self,
        processor: Architecture::ProcessorState,
        memory: Memory<Architecture::PagePermission>,
    ) -> Result<(Architecture::ProcessorStateExpectation, Seq<Assignment<Architecture::PagePermission>>), Self::Error>;

    /// Simulates applying this instruction to the given processor + memory.
    ///
    /// The processor expected afterwards is returned, memory changes are written through the hook.
    fn simulate(
        &self,
        processor: &Architecture::ProcessorState,
        memory: MemorySnapshot<'_, '_, Architecture>,
        mutation_hook: &mut impl MemorySink<Architecture>,
    ) -> (result: Result<Architecture::ProcessorStateExpectation, Self::Error>)
        ensures
            result matches Ok(expected) ==> (self.simulation(*processor, memory_of(memory.mutations@)) matches Ok((after, changes))
                && expected == after && receives(*old(mutation_hook), *final(mutation_hook), changes)),
    ;

    /// Simulates applying this instruction to the before state.
    fn simulate_state<'memory>(
        &self,
        mut before: ExecutionState<'memory, Architecture>,
    ) -> (after: Result<ExecutionStateExpectation<'memory, Architecture>, Self::Error>)
        ensures
            after matches Ok(after) ==> (self.simulation(before.processor, memory_of(before.memory_mutations@))
                matches Ok((processor, changes)) && after.processor == processor
                && memory_of(after.memory_mutations@) == apply_all(memory_of(before.memory_mutations@), changes)),
    {
        let mut mutations = Vec::new();
        let ghost none = mutations;
        let processor = self.simulate(&before.processor, before.memory(), &mut mutations)?;
        proof {
            assert(assignments(none@) =~= Seq::empty());
            assert(assignments(before.memory_mutations@ + mutations@) =~= assignments(before.memory_mutations@) + assignments(mutations@));
            lemma_apply_all_append(zeroed::<Architecture>(), assignments(before.memory_mutations@), assignments(mutations@));
            assert(none.applied_to(memory_of(before.memory_mutations@)) == memory_of(before.memory_mutations@));
            assert(mutations.applied_to(memory_of(before.memory_mutations@))
                == apply_all(memory_of(before.memory_mutations@), assignments(mutations@)));
        }
        before.memory_mutations.append(&mut mutations);
        Ok(ExecutionStateExpectation {
            processor,
            memory_mutations: before.memory_mutations,
        })
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

/// What a behavior expects an instruction to leave behind.
#[derive(Clone)]
pub struct ExecutionStateExpectation<'memory, Architecture: ProcessorArchitecture> {
    pub processor: Architecture::ProcessorStateExpectation,
    /// The changes expected to have been applied to the memory (as if it started from a fully zeroed out state), in chronological order.
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
        proof {
            Architecture::clone_is_identity();
            assert(vstd::pervasive::cloned(self.processor, cloned.processor));
            assert(assignments(cloned.memory_mutations@) =~= assignments(self.memory_mutations@)) by {
                assert forall|index: int| 0 <= index < self.memory_mutations@.len()
                    implies #[trigger] cloned.memory_mutations@[index]@ == self.memory_mutations@[index]@ by {
                    assert(vstd::pervasive::cloned(self.memory_mutations@[index], cloned.memory_mutations@[index]));
                }
            }
        }
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
}

/// Memory as the mutations applied to it from a fully zeroed out state, in chronological order.
pub struct MemorySnapshot<'snapshot, 'memory, Architecture: ProcessorArchitecture> {
    pub mutations: &'snapshot [MemoryDelta<'memory, Architecture>],
}

#[cfg(verus_keep_ghost)]
impl<Architecture: ProcessorArchitecture> vstd::std_specs::cmp::PartialEqSpecImpl for MemorySnapshot<'_, '_, Architecture> {
    open spec fn obeys_eq_spec() -> bool {
        true
    }

    /// The same byte and permission at every address.
    open spec fn eq_spec(&self, other: &Self) -> bool {
        memory_of(self.mutations@) == memory_of(other.mutations@)
    }
}

/// Two snapshots are equal when they hold the same memory, regardless of the order of mutations.
impl<Architecture: ProcessorArchitecture> PartialEq for MemorySnapshot<'_, '_, Architecture> {
    fn eq(&self, other: &Self) -> bool {
        // VERUS: no closure that captures a mutable reference, so the mutations are collected and counted.
        let mut mutations = Vec::new();
        let ghost none = mutations;
        self.diff(other, &mut mutations);
        proof {
            // A diff that sends nothing leaves the memory as it was.
            if mutations@.len() == 0 {
                let changes = choose|changes: Seq<Assignment<Architecture::PagePermission>>|
                    #![trigger apply_all(memory_of(self.mutations@), changes)] receives(none, mutations, changes)
                    && apply_all(memory_of(self.mutations@), changes) == memory_of(other.mutations@);
                assert(assignments(none@) =~= Seq::empty());
                assert(assignments(mutations@) =~= Seq::empty());
                assert(mutations.applied_to(memory_of(self.mutations@)) == apply_all(none.applied_to(memory_of(self.mutations@)), changes));
            }
        }
        mutations.is_empty()
    }
}

impl<Architecture: ProcessorArchitecture> Eq for MemorySnapshot<'_, '_, Architecture> {}

#[cfg(verus_keep_ghost)]
impl<Architecture: ProcessorArchitecture> vstd::std_specs::cmp::PartialEqSpecImpl for ExecutionState<'_, Architecture> {
    open spec fn obeys_eq_spec() -> bool {
        <Architecture::ProcessorState as vstd::std_specs::cmp::PartialEqSpec>::obeys_eq_spec()
    }

    /// The processors are `==` and the memory is the same at every address.
    open spec fn eq_spec(&self, other: &Self) -> bool {
        self.processor.eq_spec(&other.processor) && memory_of(self.memory_mutations@) == memory_of(other.memory_mutations@)
    }
}

/// Two states are equal when they hold the same processor and the same memory.
impl<Architecture: ProcessorArchitecture> PartialEq for ExecutionState<'_, Architecture> {
    fn eq(&self, other: &Self) -> bool {
        self.processor == other.processor && self.memory() == other.memory()
    }
}

impl<Architecture: ProcessorArchitecture> Eq for ExecutionState<'_, Architecture> {}

impl<'memory, Architecture: ProcessorArchitecture> ExecutionState<'memory, Architecture> {
    /// Returns the memory of this state.
    pub fn memory(&self) -> (memory: MemorySnapshot<'_, 'memory, Architecture>)
        ensures
            memory.mutations@ == self.memory_mutations@,
    {
        MemorySnapshot {
            mutations: &self.memory_mutations,
        }
    }

    /// Returns whether this observed state has the memory and every processor value the expectation holds.
    pub fn meets_expectations(
        &self,
        expectation: &ExecutionStateExpectation<'_, Architecture>,
    ) -> (meets: bool)
        ensures
            meets == (memory_of(self.memory_mutations@) == memory_of(expectation.memory_mutations@)
                && Architecture::meets(self.processor, expectation.processor)),
    {
        self.memory() == expectation.memory()
            && Architecture::meets_expectations(&self.processor, &expectation.processor)
    }
}

#[cfg(verus_keep_ghost)]
impl<Architecture: ProcessorArchitecture> vstd::std_specs::cmp::PartialEqSpecImpl for ExecutionStateExpectation<'_, Architecture> {
    open spec fn obeys_eq_spec() -> bool {
        <Architecture::ProcessorStateExpectation as vstd::std_specs::cmp::PartialEqSpec>::obeys_eq_spec()
    }

    /// The processor expectations are `==` and the memory is the same at every address.
    open spec fn eq_spec(&self, other: &Self) -> bool {
        self.processor.eq_spec(&other.processor) && memory_of(self.memory_mutations@) == memory_of(other.memory_mutations@)
    }
}

/// Two expectations are equal when they hold the same processor expectation and the same memory.
impl<Architecture: ProcessorArchitecture> PartialEq
    for ExecutionStateExpectation<'_, Architecture>
{
    fn eq(&self, other: &Self) -> bool {
        self.processor == other.processor && self.memory() == other.memory()
    }
}

impl<Architecture: ProcessorArchitecture> Eq for ExecutionStateExpectation<'_, Architecture> {}

impl<'memory, Architecture: ProcessorArchitecture>
    ExecutionStateExpectation<'memory, Architecture>
{
    /// Returns the memory this expectation holds.
    pub fn memory(&self) -> (memory: MemorySnapshot<'_, 'memory, Architecture>)
        ensures
            memory.mutations@ == self.memory_mutations@,
    {
        MemorySnapshot {
            mutations: &self.memory_mutations,
        }
    }
}

} // verus!
